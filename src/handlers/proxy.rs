use crate::clickhouse;
use crate::client::traits::*;
use crate::client::*;
use crate::firestore;
use crate::request_logs;
use crate::types::{OaiChatCompletionRequest, OaiChatCompletionResponse};
use crate::utils;
use crate::BackendConfigs;
use anyhow::Result;
use axum::{
    extract::State, http::header::HeaderMap, http::header::AUTHORIZATION, http::StatusCode,
    http::Uri, response::IntoResponse, routing::get, routing::post, Json, Router,
};
use chrono::Utc;
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
) -> Result<(StatusCode, Value)> {
    println!("OpenAI proxy request: {:?}", original_uri);

    let bearer_token = match utils::extract_bearer_token(&headers) {
        Some(token) => token,
        None => {
            return Ok((
                StatusCode::UNAUTHORIZED,
                serde_json::to_value("Missing OpenAI API_KEY")?,
            ));
        }
    };

    // construct url
    let url = url::Url::parse("https://api.openai.com/")?;
    let url = url.join(&original_uri.to_string())?;

    println!("Url: {:?}", &url.to_string());

    let request = reqwest::Client::new()
        .post(url)
        .header("Authorization", format!("Bearer {}", bearer_token))
        .json(&payload);

    // Add headers to the request
    //for (key, value) in headers.iter() {
    //    if key.as_str().to_lowercase() != "host" {
    //        request = request.header(key, value);
    //    }
    //}

    println!("Request: {:?}", request);
    //let response = request.send().await?;
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
            serde_json::to_value(response.text().await?)?,
        ));
    }
    let response_body = response.json::<Value>().await?;
    Ok((StatusCode::OK, serde_json::to_value(response_body)?))
}
