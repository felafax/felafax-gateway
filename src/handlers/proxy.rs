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

#[derive(Clone)]
pub struct Proxy {
    pub request: Option<Arc<reqwest::Request>>,
    pub response_body: Option<Arc<Value>>,
    pub backend_configs: Arc<BackendConfigs>,
}

impl Proxy {
    pub fn new(backend_configs: Arc<BackendConfigs>) -> Self {
        Self {
            request: None,
            response_body: None,
            backend_configs,
        }
    }

    // takes care of safely setting request object and by cloning
    async fn set_request(&mut self, request: &reqwest::Request) {
        let request_clone = request.try_clone();

        if let Some(request) = request_clone {
            self.request = Some(Arc::new(request));
        }
    }

    pub async fn openai_proxy(
        &mut self,
        method: Method,
        headers: HeaderMap,
        original_uri: Uri,
        payload: Value,
    ) -> Result<Response> {
        println!("OpenAI proxy request: {:?}", original_uri);

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
        self.set_request(&request).await;

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
            tokio::spawn(process_background_streaming(self.clone(), rx));

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "text/event-stream")
                .body(Body::from_stream(stream))
                .unwrap())
        } else {
            // Handle non-streaming response
            let response_body = response.json::<Value>().await?;

            // clone response body for background processing
            self.response_body = Some(Arc::new(response_body.clone()));

            Ok((StatusCode::OK, Json(response_body)).into_response())
        }
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
            // Handle final message (similar to the loop above)
            // ...
        }
    }

    // Construct the final full JSON
    let final_json = serde_json::to_value(accumulated_response).unwrap();
    println!(
        "Final accumulated JSON: {}",
        serde_json::to_string_pretty(&final_json).unwrap()
    );

    // Here you can do something with the final_json, like storing it or sending it somewhere
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
