use crate::clickhouse;
use crate::client::traits::*;
use crate::client::*;
use crate::firestore;
use crate::request_logs;
use crate::types::{OaiChatCompletionRequest, OaiChatCompletionResponse};
use crate::utils;
use crate::BackendConfigs;
use anyhow::Result;
use axum::{http::header::HeaderMap, http::StatusCode};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

async fn log_stats(
    clickhouse_client: Arc<clickhouse::Clickhouse>,
    firebase_client: Arc<firestore::Firestore>,
    status_code: StatusCode,
    felafax_token: &str,
    request: Option<&OaiChatCompletionRequest>,
    response: Option<&OaiChatCompletionResponse>,
    llm_name: Option<&str>,
    latency: u32,
    error: Option<String>,
) -> Result<()> {
    let clickhouse_client = clickhouse_client.clone();
    let mut request_logs = request_logs::RequestLogBuilder::default();
    request_logs.customer_id(Uuid::new_v4().to_string());
    request_logs.request_id(Uuid::new_v4().to_string());
    request_logs.timestamp(Utc::now().timestamp());

    request_logs.customer_id(felafax_token);

    if let Some(request) = request {
        request_logs.request(serde_json::to_string(&request)?);
    }

    if let Some(llm_name) = llm_name {
        request_logs.llm_name(llm_name.to_string());
    }

    if let Some(response) = response {
        request_logs.response(serde_json::to_string(&response)?);

        request_logs.llm_model(serde_json::to_string(&response.model)?);
        if let Some(usage) = &response.usage {
            request_logs.prompt_tokens(usage.prompt_tokens);
            request_logs.completion_tokens(usage.completion_tokens);
            request_logs.total_tokens(usage.total_tokens);
        }
    }
    request_logs.total_latency(latency);

    if let Some(error) = error {
        request_logs.error(error);
    }

    let request_logs = request_logs.build().unwrap();

    // log in background
    tokio::task::spawn(async move {
        request_logs
            .log(&clickhouse_client, &firebase_client)
            .await
            .unwrap_or_else(|e| eprintln!("Failed to log request: {:?}", e));
    });
    Ok(())
}

async fn log_and_respond(
    clickhouse_client: Arc<clickhouse::Clickhouse>,
    firebase: Arc<firestore::Firestore>,
    status_code: StatusCode,
    felafax_token: &str,
    request: Option<&OaiChatCompletionRequest>,
    response: Option<&OaiChatCompletionResponse>,
    llm_name: Option<&str>,
    latency: u32,
    error: Option<String>,
) -> Result<(StatusCode, Value)> {
    let _ = log_stats(
        clickhouse_client.clone(),
        firebase.clone(),
        status_code,
        felafax_token,
        request,
        response,
        llm_name,
        latency,
        error.clone(),
    )
    .await;

    if let Some(error) = error {
        Ok((status_code, json!({ "error": error })))
    } else {
        Ok((status_code, serde_json::to_value(response.clone())?))
    }
}

pub async fn chat_completion(
    headers: HeaderMap,
    backend_configs: Arc<BackendConfigs>,
    payload: Value,
) -> Result<(StatusCode, Value)> {
    let felafax_token = match utils::extract_bearer_token(&headers) {
        Some(token) => token,
        None => {
            return log_and_respond(
                backend_configs.clickhouse.clone(),
                backend_configs.firebase.clone(),
                StatusCode::UNAUTHORIZED,
                "",
                None,
                None,
                None,
                0,
                Some("Unauthorized: Missing or invalid token.".to_string()),
            )
            .await
        }
    };

    let customer_config = match backend_configs
        .firebase
        .get_customer_configs(&felafax_token)
        .await
    {
        Ok(Some(config)) => config,
        _ => {
            return log_and_respond(
                backend_configs.clickhouse.clone(),
                backend_configs.firebase.clone(),
                StatusCode::UNAUTHORIZED,
                &felafax_token,
                None,
                None,
                None,
                0,
                Some("Invalid felafax token".to_string()),
            )
            .await
        }
    };

    let request: OaiChatCompletionRequest = match serde_json::from_value(payload) {
        Ok(req) => req,
        Err(e) => {
            return log_and_respond(
                backend_configs.clickhouse.clone(),
                backend_configs.firebase.clone(),
                StatusCode::BAD_REQUEST,
                &felafax_token,
                None,
                None,
                None,
                0,
                Some(format!(
                    "Error while parsing request. Maybe it's not following OpenAI spec\nError: {}",
                    e.to_string()
                )),
            )
            .await
        }
    };

    let llm_response = match customer_config.selected_llm_name.as_str() {
        "claude" => {
            let api_key = customer_config
                .llm_configs
                .get("claude")
                .unwrap()
                .api_key
                .clone();
            let llm_client = claude::Claude::new().with_api_key(api_key.as_str());

            llm_client.chat(request.clone()).await
        }
        "openai" => {
            let api_key = customer_config
                .llm_configs
                .get("openai")
                .unwrap()
                .api_key
                .clone();
            let llm_client = openai::OpenAI::new().with_api_key(api_key.as_str());

            llm_client.chat(request.clone()).await
        }
        "jamba" => {
            let api_key = customer_config
                .llm_configs
                .get("jamba")
                .unwrap()
                .api_key
                .clone();
            let llm_client = mamba::Mamba::new().with_api_key(api_key.as_str());

            llm_client.chat(request.clone()).await
        }
        _ => {
            return log_and_respond(
                backend_configs.clickhouse.clone(),
                backend_configs.firebase.clone(),
                StatusCode::BAD_REQUEST,
                &felafax_token,
                Some(&request),
                None,
                None,
                0,
                Some("Invalid LLM name. Supported LLMs are: mamba, openai, claude".to_string()),
            )
            .await
        }
    };

    match llm_response {
        Ok(response) => {
            log_and_respond(
                backend_configs.clickhouse.clone(),
                backend_configs.firebase.clone(),
                StatusCode::OK,
                &felafax_token,
                Some(&request),
                Some(&response),
                Some(&customer_config.selected_llm_name),
                0,
                None,
            )
            .await
        }
        Err(e) => {
            log_and_respond(
                backend_configs.clickhouse.clone(),
                backend_configs.firebase.clone(),
                StatusCode::INTERNAL_SERVER_ERROR,
                &felafax_token,
                Some(&request),
                None,
                Some(&customer_config.selected_llm_name),
                0,
                Some(e.to_string()),
            )
            .await
        }
    }
}
