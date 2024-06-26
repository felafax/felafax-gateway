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
use serde_json::{json, Value};
use shuttle_runtime::SecretStore;
use std::sync::Arc;
use uuid::Uuid;

fn convert_err(err: reqwest::Error) -> std::io::Error {
    let err_msg = err.to_string();
    return std::io::Error::new(std::io::ErrorKind::Interrupted, err_msg);
}

pub async fn openai_proxy(
    headers: HeaderMap,
    original_uri: Uri,
    backend_configs: Arc<BackendConfigs>,
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
    let mut request = client
        .post(url)
        .header("Authorization", format!("Bearer {}", bearer_token))
        .json(&payload);

    // Check if the request is for streaming
    let is_stream = payload["stream"].as_bool().unwrap_or(false);

    if is_stream {
        request = request.header(CONTENT_TYPE, "text/event-stream");
    }

    println!("Request: {:?}", request);
    let response = request.send().await.map_err(|e| {
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
        // Handle streaming response
        let stream = response.bytes_stream().map(|result| match result {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
        });

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/event-stream")
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        // Handle non-streaming response
        let response_body = response.json::<Value>().await?;
        Ok((StatusCode::OK, Json(response_body)).into_response())
    }
}

