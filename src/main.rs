#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(async_fn_in_trait)]
// #![allow(deprecated)]

pub mod clickhouse;
pub mod client;
pub mod error;
pub mod firestore;
pub mod request_logs;
pub mod types;

use axum::{
    extract::State, http::header::HeaderMap, http::header::AUTHORIZATION, http::StatusCode,
    response::IntoResponse, routing::get, routing::post, Json, Router,
};
use chrono::Utc;
use client::traits::*;
use serde_json::{json, Value};
use shuttle_runtime::SecretStore;
use std::sync::Arc;
use types::{OaiChatCompletionRequest, OaiChatCompletionResponse};

pub struct BackendConfigs {
    secrets: SecretStore,
    firebase: firestore::Firestore,
    clickhouse: Arc<clickhouse::Clickhouse>,
}

async fn hello() -> &'static str {
    "Hello from Felafax 🦊\nSupported routes: /v1/chat/completions"
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    if let Some(auth_header) = headers.get(AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }
    None
}

async fn log_stats(
    clickhouse_client: Arc<clickhouse::Clickhouse>,
    status_code: StatusCode,
    felafax_token: &str,
    request: Option<&OaiChatCompletionRequest>,
    response: Option<&OaiChatCompletionResponse>,
    llm_name: Option<&str>,
    latency: u32,
    error: Option<String>,
) {
    let clickhouse_client = clickhouse_client.clone();
    let mut request_logs = request_logs::RequestLogBuilder::default();
    request_logs.timestamp(Utc::now().timestamp());

    request_logs.customer_id(felafax_token);

    if let Some(request) = request {
        request_logs.request(serde_json::to_string(&request).unwrap());
    }

    if let Some(llm_name) = llm_name {
        request_logs.llm_name(llm_name.to_string());
    }

    if let Some(response) = response {
        request_logs.llm_model(serde_json::to_string(&response.model).unwrap());
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

    tokio::task::spawn(async move {
        request_logs
            .log(&clickhouse_client)
            .await
            .unwrap_or_else(|e| eprintln!("Failed to log request: {:?}", e));
    });
}

async fn log_and_respond(
    clickhouse_client: Arc<clickhouse::Clickhouse>,
    status_code: StatusCode,
    felafax_token: &str,
    request: Option<&OaiChatCompletionRequest>,
    response: Option<&OaiChatCompletionResponse>,
    llm_name: Option<&str>,
    latency: u32,
    error: Option<String>,
) -> impl IntoResponse {
    log_stats(
        clickhouse_client.clone(),
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
        (status_code, Json(json!({ "error": error })))
    } else {
        (status_code, Json(serde_json::to_value(response).unwrap()))
    }
}

async fn chat_completion(
    headers: HeaderMap,
    State(backend_configs): State<Arc<BackendConfigs>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let felafax_token = match extract_bearer_token(&headers) {
        Some(token) => token,
        None => {
            return log_and_respond(
                backend_configs.clickhouse.clone(),
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
            let llm_client = client::claude::Claude::new().with_api_key(api_key.as_str());

            llm_client.chat(request.clone()).await
        }
        "openai" => {
            let api_key = customer_config
                .llm_configs
                .get("openai")
                .unwrap()
                .api_key
                .clone();
            let llm_client = client::openai::OpenAI::new().with_api_key(api_key.as_str());

            llm_client.chat(request.clone()).await
        }
        "jamba" => {
            let api_key = customer_config
                .llm_configs
                .get("jamba")
                .unwrap()
                .api_key
                .clone();
            let llm_client = client::mamba::Mamba::new().with_api_key(api_key.as_str());

            llm_client.chat(request.clone()).await
        }
        _ => {
            return log_and_respond(
                backend_configs.clickhouse.clone(),
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

async fn chat_completion_test(
    headers: HeaderMap,
    State(backend_configs): State<Arc<BackendConfigs>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // let's parse the request payload into OpenAI spec
    let request: OaiChatCompletionRequest = match serde_json::from_value(payload) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("Failed to deserialize request: {:?}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    json!({ "error": format!("Error while parsing request. Maybe it's not following OpenAI spec\nError: {} ", e.to_string()) }),
                ),
            );
        }
    };

    let api_key = backend_configs
        .secrets
        .get("CLAUDE_API_KEY")
        .unwrap_or_else(|| panic!("Error: API_KEY not found in secrets."));

    let llm_client = client::claude::Claude::new().with_api_key(api_key.as_str());

    let response = match llm_client.chat(request.clone()).await {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Failed to get completion: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };
    (
        StatusCode::OK,
        Json(serde_json::to_value(response).unwrap()),
    )
}

#[shuttle_runtime::main]
async fn main(#[shuttle_runtime::Secrets] secrets: SecretStore) -> shuttle_axum::ShuttleAxum {
    // firebase init
    let firebase = firestore::Firestore::new(
        &secrets
            .get("FIREBASE_PROJECT_ID")
            .unwrap_or_else(|| panic!("Error: FIREBASE_PROJECT_ID not found in secrets.")),
    );
    firebase
        .init()
        .await
        .unwrap_or_else(|e| panic!("Failed to initialise firestore: {:?}", e));

    // init clickhouse
    let click_house_url = secrets
        .get("CLICKHOUSE_URL")
        .unwrap_or_else(|| panic!("Error: CLICKHOUSE_URL not found in secrets."));
    let clickhouse_username = secrets
        .get("CLICKHOUSE_USERNAME")
        .unwrap_or_else(|| panic!("Error: CLICKHOUSE_USER not found in secrets."));
    let clickhouse_password = &secrets
        .get("CLICKHOUSE_PASSWORD")
        .unwrap_or_else(|| panic!("Error: CLICKHOUSE_PASSWORD not found in secrets."));
    let clickhouse_database = secrets
        .get("CLICKHOUSE_DATABASE")
        .unwrap_or_else(|| panic!("Error: CLICKHOUSE_DATABASE not found in secrets."));

    let clickhouse_client = Arc::new(clickhouse::Clickhouse::new(
        &click_house_url,
        &clickhouse_username,
        &clickhouse_password,
        &clickhouse_database,
    ));

    let backend_configs = BackendConfigs {
        secrets,
        firebase,
        clickhouse: clickhouse_client,
    };
    let backend_configs = Arc::new(backend_configs);

    let router = Router::new()
        .route("/", get(hello))
        .route("/v1/chat/completions", post(chat_completion))
        .with_state(backend_configs);

    Ok(router.into())
}
