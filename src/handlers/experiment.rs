use crate::handlers::openai_proxy::*;
use crate::BackendConfigs;
use anyhow::Result;
use axum::{
    body::Body,
    extract::State,
    http::{
        header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE},
        Method, StatusCode, Uri,
    },
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json;
use std::sync::Arc;

pub struct Experiment {
    backend_configs: Arc<BackendConfigs>,
}

impl Experiment {
    pub fn new(backend_configs: Arc<BackendConfigs>) -> Self {
        Self { backend_configs }
    }

    pub fn override_payload(
        &self,
        payload: serde_json::Value,
        headers: HeaderMap,
        token: &str,
    ) -> Result<serde_json::Value> {
        // check if headers has new roll_out prompt
        // check the % roll-out for users
        // flip a weighted coin
        // return new overloaded payload
        println!("Headers: {:?}", headers);
        println!("Payload: {:?}", payload);

        if headers.contains_key("felafax_proxy") {
            println!("Felafax Proxy found");
            // get felafax_proxy header
            let felafax_proxy = headers.get("felafax_proxy").unwrap();
            let felafax_proxy_str = felafax_proxy.to_str()?;
            let felafax_proxy: FelafaxProxy = serde_json::from_str(felafax_proxy_str)?;

            // get request contents
            let mut request: CompletionRequest = serde_json::from_value(payload.clone())?;

            // in the messages, find the first prompt that matches the role type and replace with
            // felafax override
            for message in request.messages.iter_mut() {
                if message.role == felafax_proxy.system_prompt.role {
                    message.content = felafax_proxy.system_prompt.content.clone();
                }
            }

            println!("override request: {:?}", request);

            // serialise request to value and return
            return Ok(serde_json::to_value(request)?);
        }
        Ok(payload)
    }
}

#[derive(Deserialize, Debug)]
pub struct FelafaxProxy {
    pub system_prompt: SystemPrompt,
    pub user_id: String,
}

#[derive(Deserialize, Debug)]
pub struct SystemPrompt {
    pub role: String,
    pub content: String,
}
