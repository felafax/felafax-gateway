#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(async_fn_in_trait)]
// #![allow(deprecated)]

pub mod client;
pub mod error;
pub mod types;

use axum::{
    extract::State, http::StatusCode, response::IntoResponse, routing::get, routing::post, Json,
    Router,
};
use client::traits::*;
use serde_json::{json, Value};
use shuttle_runtime::SecretStore;
use std::sync::Arc;
use types::OaiChatCompletionRequest;

struct BackendConfigs {
    secrets: SecretStore,
}

async fn hello() -> &'static str {
    "Hello from Felafax 🦊\nSupported routes: /v1/chat/completions"
}

async fn chat_completion(
    State(backend_configs): State<Arc<BackendConfigs>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    println!("Received payload: {:?}", payload);

    let request: OaiChatCompletionRequest = match serde_json::from_value(payload) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("Failed to deserialize request: {:?}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };

    println!("Request: {:?}", request);
    let mamba_api_key = backend_configs
        .secrets
        .get("MAMBA_API_KEY")
        .unwrap_or_else(|| panic!("Error: MAMBA_API_KEY not found in secrets."));

    // Mamba
    let mamba = client::mamba::Mamba::new().with_api_key(mamba_api_key.as_str());

    let openai_response = match mamba.chat(request.clone()).await {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Failed to get completion: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            );
        }
    };
    println!("\n\nOpenAI response: {:?}\n\n", openai_response);

    (
        StatusCode::OK,
        Json(serde_json::to_value(openai_response).unwrap()),
    )
}

#[shuttle_runtime::main]
async fn main(#[shuttle_runtime::Secrets] secrets: SecretStore) -> shuttle_axum::ShuttleAxum {
    let backend_configs = BackendConfigs { secrets };
    let backend_configs = Arc::new(backend_configs);

    let router = Router::new()
        .route("/", get(hello))
        .route("/v1/chat/completions", post(chat_completion))
        .with_state(backend_configs);

    Ok(router.into())
}
