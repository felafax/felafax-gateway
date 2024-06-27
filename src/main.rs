#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(async_fn_in_trait)]
// #![allow(deprecated)]

pub mod clickhouse;
pub mod client;
pub mod error;
pub mod firestore;
pub mod handlers;
pub mod request_logs;
pub mod types;
pub mod utils;

use axum::{
    extract::OriginalUri, extract::State, http::header::HeaderMap, http::header::AUTHORIZATION,
    http::Method, http::StatusCode, response::IntoResponse, routing::any, routing::get,
    routing::post, Json, Router,
};
use chrono::Utc;
use client::traits::*;
use serde_json::{json, Value};
use shuttle_runtime::SecretStore;
use std::sync::Arc;
use types::{OaiChatCompletionRequest, OaiChatCompletionResponse};
use uuid::Uuid;

#[derive(Clone)]
pub struct BackendConfigs {
    secrets: SecretStore,
    firebase: Arc<firestore::Firestore>,
    clickhouse: Arc<clickhouse::Clickhouse>,
}

async fn hello() -> &'static str {
    "Hello from Felafax 🦊\nSupported routes: /v1/chat/completions"
}

pub async fn translate_chat_completion(
    headers: HeaderMap,
    State(backend_configs): State<Arc<BackendConfigs>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let result = handlers::translate::chat_completion(headers, backend_configs, payload).await;
    if result.is_ok() {
        let (status_code, value) = result.unwrap();
        let response = Json(value);
        (status_code, response)
    } else {
        let status_code = StatusCode::INTERNAL_SERVER_ERROR;
        (status_code, Json(json!("Internal server error")))
    }
}

pub async fn proxy(
    method: Method,
    headers: HeaderMap,
    OriginalUri(original_uri): OriginalUri,
    State(backend_configs): State<Arc<BackendConfigs>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let result = handlers::openai_proxy::openai_proxy(
        method,
        headers,
        original_uri,
        payload,
        backend_configs.clone(),
    )
    .await;
    if result.is_ok() {
        let response = result.unwrap();
        (StatusCode::OK, response).into_response()
    } else {
        (StatusCode::OK, Json(json!("{}"))).into_response()
    }
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

    let firebase = Arc::new(firebase);

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
        .route(
            "/translate/v1/chat/completions",
            post(translate_chat_completion),
        )
        .fallback(any(proxy))
        .with_state(backend_configs);

    Ok(router.into())
}

