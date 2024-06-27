use crate::clickhouse;
use crate::client::traits::*;
use crate::client::*;
use crate::firestore;
use crate::request_logs;
use crate::types::{OaiChatCompletionRequest, OaiChatCompletionResponse};
use crate::utils;
use crate::BackendConfigs;
use anyhow::Result;
use axum::body::Body;
use axum::{
    extract::State,
    http::header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE},
    http::Method,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    routing::post,
    Json, Router,
};
use bytes::Bytes;
use chrono::Utc;
use derive_builder::Builder;
use futures::stream::StreamExt;
use native_tls::TlsConnector as NativeTlsConnector;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shuttle_runtime::SecretStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Builder, Default)]
#[builder(setter(into, strip_option), default)]
#[builder(pattern = "mutable")]
pub struct Proxy {
    pub payload: Option<Value>,
    pub backend_configs: Option<Arc<BackendConfigs>>,
    pub felafax_token: Option<String>,
    pub headers: Option<HeaderMap>,
}

impl Proxy {}

pub async fn openai_proxy(
    method: Method,
    headers: HeaderMap,
    original_uri: Uri,
    payload: Value,
    backend_configs: Arc<BackendConfigs>,
) -> Result<Response> {
    println!("OpenAI proxy request: {:?}", original_uri);
    let mut proxy_instance = ProxyBuilder::default();

    let bearer_token = match utils::extract_bearer_token(&headers) {
        Some(token) => token,
        None => {
            return Ok((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing OpenAI API_KEY"})),
            )
                .into_response());
        }
    };

    // setup proxy instance
    proxy_instance.felafax_token(&bearer_token);
    proxy_instance.payload(payload.clone());
    proxy_instance.backend_configs(backend_configs);
    proxy_instance.headers(headers);

    // construct url
    let url = url::Url::parse("https://api.openai.com/")?;
    let url = url.join(&original_uri.to_string())?;

    println!("Url: {:?}", &url.to_string());

    let client = reqwest::Client::new();
    let request = match method {
        Method::GET => client.get(url),
        Method::POST => client.post(url),
        Method::PUT => client.put(url),
        Method::DELETE => client.delete(url),
        _ => {
            return Ok((
                StatusCode::METHOD_NOT_ALLOWED,
                Json(json!({"error": "Method not allowed"})),
            )
                .into_response());
        }
    };

    let mut request = request
        .header("Authorization", format!("Bearer {}", bearer_token))
        .json(&payload);

    // Check if the request is for streaming
    let is_stream = payload["stream"].as_bool().unwrap_or(false);

    if is_stream {
        request = request.header(CONTENT_TYPE, "text/event-stream");
    }

    let request = request.build()?;

    println!("Request: {:?}", request);
    let response = client.execute(request).await.map_err(|e| {
        println!("Error sending request: {:?}", e);
        e
    })?;

    println!("Response: {:?}", response);

    if !response.status().is_success() {
        tracing::error!(
            status = response.status().as_u16(),
            "Failed to make completion request to OpenAI"
        );
        return Ok((
            StatusCode::OK,
            Json(json!({"error": response.text().await?})),
        )
            .into_response());
    }

    if is_stream {
        let (tx, rx): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) = mpsc::channel(100);
        // Handle streaming response
        let stream = response.bytes_stream().map(move |result| match result {
            Ok(bytes) => {
                // clone and send bytes for background processing
                let bytes_clone = Bytes::copy_from_slice(&bytes);
                let _ = tx.try_send(bytes_clone).map_err(|e| {
                    println!("Error sending bytes: {:?}", e);
                });

                Ok(bytes)
            }
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
        });

        // Spawn the background processing task
        // TODO: handle unwrap errors here
        let proxy_instance = proxy_instance.build().unwrap();
        tokio::spawn(process_background_streaming(proxy_instance, rx));

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/event-stream")
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        // Handle non-streaming response
        let response_body = response.json::<Value>().await?;

        // clone response body for background processing
        let proxy_instance = proxy_instance.build().unwrap();
        tokio::spawn(process_background(proxy_instance, response_body.clone()));

        Ok((StatusCode::OK, Json(response_body)).into_response())
    }
}

async fn log_stats(
    proxy: Proxy,
    response: Option<String>,
    usage: Option<Usage>,
    error: Option<String>,
) {
    let mut request_logs = request_logs::RequestLogBuilder::default();
    request_logs.id(Uuid::new_v4().to_string());
    request_logs.timestamp(Utc::now().timestamp());

    if let Some(token) = proxy.felafax_token {
        request_logs.customer_id(token);
    }

    if let Some(request) = proxy.payload {
        request_logs.request(request.to_string());
    }

    if let Some(response) = response {
        request_logs.response(response);
    }
    if let Some(usage) = usage {
        request_logs.prompt_tokens(usage.prompt_tokens);
        request_logs.completion_tokens(usage.completion_tokens);
        request_logs.total_tokens(usage.total_tokens);
    }

    //request_logs.total_latency(0);
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
}

async fn process_background(proxy_instance: Proxy, response_body: Value) {
    // Process the response body
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

        // Process complete messages
        while let Some(end_pos) = buffer.find("\n\n") {
            let message = buffer[..end_pos].to_string();
            buffer = buffer[end_pos + 2..].to_string();

            if let Some(response) = process_message(&message) {
                println!("Processed message: {:?}", response);

                // Accumulate the response
                if accumulated_response.id.is_empty() {
                    accumulated_response.id = response.id;
                    accumulated_response.object = response.object;
                    accumulated_response.model = response.model;
                }

                // Accumulate choices
                for choice in response.choices {
                    if let Some(content) = choice.delta.content {
                        accumulated_content.push_str(&content);
                    }
                    if choice.finish_reason.is_some() {
                        accumulated_response.choices.push(CompletionChoiceResponse {
                            delta: CompletionDeltaResponse {
                                role: Some("assistant".to_string()),
                                content: Some(accumulated_content.clone()),
                            },
                            finish_reason: choice.finish_reason,
                        });
                    }
                }

                // Update usage if available
                if let Some(usage) = response.usage {
                    accumulated_response.usage = Some(usage);
                }
            }
        }
    }

    // Process any remaining data in the buffer
    if !buffer.is_empty() {
        if let Some(response) = process_message(&buffer) {
            println!("Final message: {:?}", response);

            // Accumulate the response
            if accumulated_response.id.is_empty() {
                accumulated_response.id = response.id;
                accumulated_response.object = response.object;
                accumulated_response.model = response.model;
            }

            // Accumulate choices
            for choice in response.choices {
                if let Some(content) = choice.delta.content {
                    accumulated_content.push_str(&content);
                }
                if choice.finish_reason.is_some() {
                    accumulated_response.choices.push(CompletionChoiceResponse {
                        delta: CompletionDeltaResponse {
                            role: Some("assistant".to_string()),
                            content: Some(accumulated_content.clone()),
                        },
                        finish_reason: choice.finish_reason,
                    });
                }
            }

            // Update usage if available
            if let Some(usage) = response.usage {
                accumulated_response.usage = Some(usage);
            }
        }
    }

    // Construct the final full JSON
    let usage = accumulated_response.usage.clone();
    let final_json = serde_json::to_value(accumulated_response).unwrap();
    println!(
        "Final accumulated JSON: {}",
        serde_json::to_string_pretty(&final_json).unwrap()
    );

    // Here you can do something with the final_json, like storing it or sending it somewhere
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
// Request
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CompletionRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
}

// for stream choices has delta and for non-stream it is a vec of messages
#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
#[serde(default)]
pub struct OpenAIResponse<T> {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub choices: Vec<T>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionDeltaResponse {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionChoiceResponse {
    #[serde(default)]
    delta: CompletionDeltaResponse,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
#[serde(default)]
pub struct ChoiceMessage {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub message: Message,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
#[serde(default)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

// You'll need to define OaiMessage as well
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Message {
    // Define the fields you need
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: String,
    // Add other fields as needed
}
