#![allow(unused_variables)]
#![allow(dead_code)]
//#![allow(unused_imports)]
#![allow(async_fn_in_trait)]
#![allow(deprecated)]

pub mod clickhouse;
pub mod client;
pub mod error;
pub mod firestore;
pub mod handlers;
pub mod request_logs;
pub mod types;
pub mod utils;

use axum::{
    extract::OriginalUri, extract::State, http::header::HeaderMap, http::Method, http::StatusCode,
    response::IntoResponse, routing::any, routing::get, routing::post, Json, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct BackendConfigs {
    firebase: Arc<firestore::Firestore>,
    clickhouse: Arc<clickhouse::Clickhouse>,
}

async fn hello() -> &'static str {
    "Hello from Felafax 🦊 Supported routes: /v1/chat/completions"
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

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv::dotenv().ok();

    // Firebase init
    let firebase_key = std::env::var("FIREBASE_SERVICE_ACCOUNT_KEY").unwrap_or_else(|_| {
        std::env::var("GOOGLE_APPLICATION_CREDENTIALS").unwrap_or_else(|_| {
            panic!("Error: FIREBASE_SERVICE_ACCOUNT_KEY or GOOGLE_APPLICATION_CREDENTIALS not found in environment.")
        })
    });

    let firebase = firestore::Firestore::new(
        &std::env::var("FIREBASE_PROJECT_ID")
            .expect("Error: FIREBASE_PROJECT_ID not found in environment."),
        &firebase_key,
    );
    firebase
        .init()
        .await
        .unwrap_or_else(|e| panic!("Failed to initialise firestore: {:?}", e));

    let firebase = Arc::new(firebase);

    // Init clickhouse
    let click_house_url =
        std::env::var("CLICKHOUSE_URL").expect("Error: CLICKHOUSE_URL not found in environment.");
    let clickhouse_username = std::env::var("CLICKHOUSE_USERNAME")
        .expect("Error: CLICKHOUSE_USERNAME not found in environment.");
    let clickhouse_password = std::env::var("CLICKHOUSE_PASSWORD")
        .expect("Error: CLICKHOUSE_PASSWORD not found in environment.");
    let clickhouse_database = std::env::var("CLICKHOUSE_DATABASE")
        .expect("Error: CLICKHOUSE_DATABASE not found in environment.");

    let clickhouse_client = Arc::new(clickhouse::Clickhouse::new(
        &click_house_url,
        &clickhouse_username,
        &clickhouse_password,
        &clickhouse_database,
    ));

    let backend_configs = BackendConfigs {
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

    // Run the server
    //let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    //println!("Listening on {}", addr);
    //axum::Server::bind(&addr)
    //    .serve(router.into_make_service())
    //    .await
    //    .unwrap();
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    println!("Listening on {}", port);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    axum::serve(listener, router).await.unwrap();
}
