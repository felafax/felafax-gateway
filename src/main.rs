#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(async_fn_in_trait)]
// #![allow(deprecated)]

pub mod client;
pub mod error;
pub mod firestore;
pub mod types;

use axum::{
    extract::State, http::header::HeaderMap, http::header::AUTHORIZATION, http::StatusCode,
    response::IntoResponse, routing::get, routing::post, Json, Router,
};
use client::traits::*;
use serde_json::{json, Value};
use shuttle_runtime::SecretStore;
use std::sync::Arc;
use types::OaiChatCompletionRequest;

struct BackendConfigs {
    secrets: SecretStore,
    firebase: firestore::Firestore,
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

async fn chat_completion(
    headers: HeaderMap,
    State(backend_configs): State<Arc<BackendConfigs>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // get the customer felfax token from header
    let felafax_token = extract_bearer_token(&headers);

    if felafax_token.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Unauthorized: Missing or invalid token." })),
        );
    }

    println!("felafax_token: {:?}", felafax_token);

    let customer_config = backend_configs
        .firebase
        .get_customer_configs(&felafax_token.unwrap())
        .await;

    println!("customer_config: {:?}", customer_config);

    if customer_config.is_err() || customer_config.as_ref().unwrap().is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid felafax token" })),
        );
    }
    let customer_config = customer_config.unwrap().unwrap();
    println!("customer_config: {:?}", customer_config);

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

    //let mamba_api_key = backend_configs
    //    .secrets
    //    .get("MAMBA_API_KEY")
    //    .unwrap_or_else(|| panic!("Error: MAMBA_API_KEY not found in secrets."));

    if customer_config.selected_llm_name == "claude" {
        let api_key = customer_config
            .llm_configs
            .get("claude")
            .unwrap()
            .api_key
            .clone();
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
        println!("\n\nClaude response: {:?}\n\n", response);

        (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
    } else if customer_config.selected_llm_name == "openai" {
        let api_key = customer_config
            .llm_configs
            .get("openai")
            .unwrap()
            .api_key
            .clone();
        let llm_client = client::openai::OpenAI::new().with_api_key(api_key.as_str());
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
        println!("\n\nOpenAI response: {:?}\n\n", response);

        (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
    } else if customer_config.selected_llm_name == "jamba" {
        let api_key = customer_config
            .llm_configs
            .get("jamba")
            .unwrap()
            .api_key
            .clone();
        let llm_client = client::mamba::Mamba::new().with_api_key(api_key.as_str());
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
        println!("\n\nJamba response: {:?}\n\n", response);

        (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap()),
        )
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid LLM name. Supported LLMs are: mamba, openai, claude"})),
        );
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

    let backend_configs = BackendConfigs { secrets, firebase };
    let backend_configs = Arc::new(backend_configs);

    let router = Router::new()
        .route("/", get(hello))
        .route("/v1/chat/completions", post(chat_completion))
        .with_state(backend_configs);

    Ok(router.into())
}
