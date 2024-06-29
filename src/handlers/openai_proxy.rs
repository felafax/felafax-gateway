use anyhow::Result;
use axum::{
    body::Body,
    extract::State,
    http::{
        header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE},
        Method, StatusCode, Uri,
    },
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use chrono::Utc;
use derive_builder::Builder;
use futures::stream::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    clickhouse,
    client::traits::*,
    firestore,
    handlers::experiment,
    request_logs,
    types::{OaiChatCompletionRequest, OaiChatCompletionResponse},
    utils, BackendConfigs,
};

#[derive(Builder, Default)]
#[builder(setter(into, strip_option), default)]
pub struct Proxy {
    request: Option<Value>,
    backend_configs: Option<Arc<BackendConfigs>>,
    bearer_token: Option<String>,
    headers: Option<HeaderMap>,
}

pub async fn openai_proxy(
    method: Method,
    headers: HeaderMap,
    original_uri: Uri,
    payload: Value,
    backend_configs: Arc<BackendConfigs>,
) -> Result<Response> {
    println!("OpenAI proxy request: {:?}", original_uri);
    let mut proxy_instance = ProxyBuilder::default();
    proxy_instance.request(payload.clone());

    let bearer_token = match utils::extract_bearer_token(&headers) {
        Some(token) => token,
        None => return Ok(unauthorized_response()),
    };

    // experimentation override
    let mut payload = payload;
    let experiment = experiment::Experiment::new(backend_configs.clone());
    match experiment.override_payload(payload.clone(), headers.clone(), &bearer_token) {
        Ok(new_payload) => {
            payload = new_payload;
        }
        Err(e) => {
            eprintln!("Error overriding payload  {:?}", e);
        }
    }

    // construct logging object
    let proxy_instance = proxy_instance
        .bearer_token(&bearer_token)
        .request(payload.clone())
        .backend_configs(backend_configs)
        .headers(headers)
        .build()?;

    let url = construct_url(&original_uri)?;
    println!("Url: {:?}", &url.to_string());

    let client = Client::new();
    let request = build_request(&client, method, url, &bearer_token, &payload)?;

    let is_stream = payload["stream"].as_bool().unwrap_or(false);
    let response = client.execute(request).await?;

    println!("Response: {:?}", response);

    if !response.status().is_success() {
        return Ok(error_response(response).await);
    }

    if is_stream {
        handle_streaming_response(response, proxy_instance).await
    } else {
        handle_non_streaming_response(response, proxy_instance).await
    }
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "Missing OpenAI API_KEY"})),
    )
        .into_response()
}

fn construct_url(original_uri: &Uri) -> Result<url::Url> {
    let base_url = url::Url::parse("https://api.openai.com/")?;
    Ok(base_url.join(&original_uri.to_string())?)
}

fn build_request(
    client: &Client,
    method: Method,
    url: url::Url,
    bearer_token: &str,
    payload: &Value,
) -> Result<reqwest::Request> {
    let mut request = match method {
        Method::GET => client.get(url),
        Method::POST => client.post(url),
        Method::PUT => client.put(url),
        Method::DELETE => client.delete(url),
        _ => return Err(anyhow::anyhow!("Method not allowed")),
    };

    request = request
        .header("Authorization", format!("Bearer {}", bearer_token))
        .json(payload);

    if payload["stream"].as_bool().unwrap_or(false) {
        request = request.header(CONTENT_TYPE, "text/event-stream");
    }

    Ok(request.build()?)
}

async fn error_response(response: reqwest::Response) -> Response {
    tracing::error!(
        status = response.status().as_u16(),
        "Failed to make completion request to OpenAI"
    );
    (
        StatusCode::OK,
        Json(json!({"error": response.text().await.unwrap_or_default()})),
    )
        .into_response()
}

async fn handle_streaming_response(
    response: reqwest::Response,
    proxy_instance: Proxy,
) -> Result<Response> {
    let (tx, rx) = mpsc::channel(100);
    let stream = response.bytes_stream().map(move |result| match result {
        Ok(bytes) => {
            let bytes_clone = bytes.clone();
            let _ = tx.try_send(bytes_clone);
            Ok(bytes)
        }
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
    });

    tokio::spawn(process_background_streaming(proxy_instance, rx));

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap())
}

async fn handle_non_streaming_response(
    response: reqwest::Response,
    proxy_instance: Proxy,
) -> Result<Response> {
    let response_body = response.json::<Value>().await?;
    tokio::spawn(process_background(proxy_instance, response_body.clone()));
    Ok((StatusCode::OK, Json(response_body)).into_response())
}

async fn log_stats(
    proxy: Proxy,
    response: Option<String>,
    usage: Option<Usage>,
    error: Option<String>,
) {
    // run this in the background
    tokio::spawn(async move {
        let mut request_logs = request_logs::RequestLogBuilder::default();
        let request_logs = request_logs
            .id(Uuid::new_v4().to_string())
            .timestamp(Utc::now().timestamp())
            .customer_id(proxy.bearer_token.unwrap_or_default())
            .request(proxy.request.map(|r| r.to_string()).unwrap_or_default())
            .response(response.unwrap_or_default());

        if let Some(usage) = usage {
            request_logs
                .prompt_tokens(usage.prompt_tokens)
                .completion_tokens(usage.completion_tokens)
                .total_tokens(usage.total_tokens);
        }

        if let Some(error) = error {
            request_logs.error(error);
        }

        let request_logs = request_logs.build().unwrap();
        if let Some(backend_configs) = &proxy.backend_configs {
            let clickhouse_client = backend_configs.clickhouse.clone();
            let firebase_client = backend_configs.firebase.clone();
            request_logs
                .log(&clickhouse_client, &firebase_client)
                .await
                .unwrap_or_else(|e| eprintln!("Failed to log request: {:?}", e));
        }
    });
}

async fn process_background(proxy_instance: Proxy, response_body: Value) {
    if let Ok(response) = serde_json::from_value::<OpenAIResponse<ChoiceMessage>>(response_body) {
        println!("Processed message: {:?}", response);
        let response_str = serde_json::to_string(&response).unwrap();
        let usage = response.usage;
        tokio::spawn(async move {
            log_stats(proxy_instance, Some(response_str), usage, None).await;
        });
    } else {
        println!("Failed to parse message");
    }
}

async fn process_background_streaming(proxy_instance: Proxy, mut rx: mpsc::Receiver<Bytes>) {
    let mut buffer = String::new();
    let mut accumulated_response = OpenAIResponse::<CompletionChoiceResponse>::default();
    let mut accumulated_content = String::new();

    while let Some(chunk) = rx.recv().await {
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(end_pos) = buffer.find("\n\n") {
            let message = buffer[..end_pos].to_string();
            buffer = buffer[end_pos + 2..].to_string();

            if let Some(response) = process_message(&message) {
                accumulate_response(
                    &mut accumulated_response,
                    &response,
                    &mut accumulated_content,
                );
            }
        }
    }

    if !buffer.is_empty() {
        if let Some(response) = process_message(&buffer) {
            accumulate_response(
                &mut accumulated_response,
                &response,
                &mut accumulated_content,
            );
        }
    }

    let usage = accumulated_response.usage.clone();
    let final_json = serde_json::to_value(accumulated_response).unwrap();
    println!(
        "Final accumulated JSON: {}",
        serde_json::to_string_pretty(&final_json).unwrap()
    );

    let response_str = serde_json::to_string(&final_json).unwrap();
    tokio::spawn(async move {
        log_stats(proxy_instance, Some(response_str), usage, None).await;
    });
}

fn process_message(message: &str) -> Option<OpenAIResponse<CompletionChoiceResponse>> {
    if message.starts_with("data: ") {
        let data = &message[6..];
        if data.trim() == "[DONE]" {
            return None;
        }
        if let Ok(response) = serde_json::from_str::<OpenAIResponse<CompletionChoiceResponse>>(data)
        {
            return Some(response);
        } else {
            println!("Failed to parse message: {:?}", data);
        }
    }
    None
}

fn accumulate_response(
    accumulated_response: &mut OpenAIResponse<CompletionChoiceResponse>,
    response: &OpenAIResponse<CompletionChoiceResponse>,
    accumulated_content: &mut String,
) {
    if accumulated_response.id.is_empty() {
        accumulated_response.id = response.id.clone();
        accumulated_response.object = response.object.clone();
        accumulated_response.model = response.model.clone();
    }

    for choice in &response.choices {
        if let Some(content) = &choice.delta.content {
            accumulated_content.push_str(content);
        }
        if choice.finish_reason.is_some() {
            accumulated_response.choices.push(CompletionChoiceResponse {
                delta: CompletionDeltaResponse {
                    role: Some("assistant".to_string()),
                    content: Some(accumulated_content.clone()),
                },
                finish_reason: choice.finish_reason.clone(),
            });
        }
    }

    if let Some(usage) = &response.usage {
        accumulated_response.usage = Some(usage.clone());
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
#[serde(default)]
pub struct OpenAIResponse<T> {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<T>,
    pub usage: Option<Usage>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompletionDeltaResponse {
    content: Option<String>,
    pub role: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompletionChoiceResponse {
    delta: CompletionDeltaResponse,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
#[serde(default)]
pub struct ChoiceMessage {
    pub index: u32,
    pub message: Message,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
#[serde(default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Message {
    pub role: String,
    pub content: String,
}
