use crate::handlers::openai_proxy::CompletionRequest;
use crate::BackendConfigs;
use anyhow::{Context, Result};
use axum::http::header::HeaderMap;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::Arc;

pub struct Experiment {
    backend_configs: Arc<BackendConfigs>,
}

impl Experiment {
    pub fn new(backend_configs: Arc<BackendConfigs>) -> Self {
        Self { backend_configs }
    }

    pub async fn override_payload(
        &self,
        payload: serde_json::Value,
        headers: &HeaderMap,
    ) -> Result<serde_json::Value> {
        if let Some(felafax_proxy) = self.extract_felafax_proxy(headers)? {
            let mut request: CompletionRequest = serde_json::from_value(payload)?;

            let roll_out_percentage = self.get_roll_out_percentage(felafax_proxy.clone()).await?;

            if Self::weighted_coin_flip(roll_out_percentage) {
                if let Some(system_prompt) = felafax_proxy.system_prompt {
                    println!(
                        "Overriding system prompt with rollout percentage: {}",
                        roll_out_percentage
                    );
                    self.update_system_prompt(&mut request, &system_prompt);
                }
            } else {
                println!(
                    "Rollout percentage coin flip failed: {}",
                    roll_out_percentage
                );
            }

            println!("Override request: {:?}", request);
            Ok(serde_json::to_value(request)?)
        } else {
            Ok(payload)
        }
    }

    async fn get_roll_out_percentage(&self, felafax_proxy: FelafaxProxy) -> Result<f64> {
        let firebase_client = &self.backend_configs.firebase;

        if let (Some(token), Some(rollout_id)) =
            (&felafax_proxy.felafax_token, &felafax_proxy.rollout_id)
        {
            if let Some(user_id) = firebase_client.get_user_id(token).await? {
                if let Some(percentage) = firebase_client
                    .get_roll_out_percentage(&user_id, rollout_id)
                    .await?
                {
                    return Ok(percentage as f64);
                }
            }
        }

        Ok(0.0)
    }

    pub fn weighted_coin_flip(probability: f64) -> bool {
        if !(0.0..=100.0).contains(&probability) {
            //Probability must be between 0.0 and 100.0;
            return false;
        }

        let mut rng = rand::thread_rng();
        rng.gen_bool(probability / 100.0)
    }

    fn extract_felafax_proxy(&self, headers: &HeaderMap) -> Result<Option<FelafaxProxy>> {
        headers
            .get("felafax_proxy")
            .map(|header| {
                let header_str = header.to_str().context("Invalid header value")?;
                serde_json::from_str(header_str).context("Failed to parse FelafaxProxy")
            })
            .transpose()
    }

    fn update_system_prompt(&self, request: &mut CompletionRequest, system_prompt: &SystemPrompt) {
        if let Some(message) = request
            .messages
            .iter_mut()
            .find(|m| m.role == system_prompt.role)
        {
            message.content = system_prompt.content.clone();
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FelafaxProxy {
    pub system_prompt: Option<SystemPrompt>,
    pub felafax_token: Option<String>,
    pub rollout_id: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemPrompt {
    pub role: String,
    pub content: String,
}
