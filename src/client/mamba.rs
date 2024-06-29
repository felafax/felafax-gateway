use super::traits::ChatTrait;
use crate::types::LLMConfig;
use crate::types::*;
use anyhow::bail;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

pub struct Mamba {
    api_key: String,
}

impl LLMConfig for Mamba {
    fn get_api_key(&self) -> String {
        self.api_key.clone()
    }

    fn set_api_key(&mut self, api_key: &str) {
        self.api_key = api_key.to_string();
    }

    fn get_base_url(&self) -> String {
        "https://api.ai21.com/studio".to_string()
    }

    fn get_name(&self) -> String {
        "Mamba".to_string()
    }

    fn get_default_model(&self) -> String {
        "jamba-instruct-preview".to_string()
    }
}

impl Mamba {
    pub fn new() -> Self {
        Self {
            api_key: "".to_string(),
        }
    }

    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_key = api_key.to_string();
        self
    }
}

impl ChatTrait for Mamba {
    async fn chat(&self, request: OaiChatCompletionRequest) -> Result<OaiChatCompletionResponse> {
        //convert request
        let mut mamba_request: ChatRequest = request.into();
        mamba_request.model = self.get_default_model();

        let http_request = reqwest::Client::new()
            .post(format!(
                "{url}/v1/chat/completions",
                url = self.get_base_url()
            ))
            .header("Authorization", format!("Bearer {}", self.get_api_key()))
            .json(&mamba_request);

        println!("MAMBA REQUEST: {:?}", http_request);

        let http_response = http_request.send().await?;

        if !http_response.status().is_success() {
            let status = http_response.status().as_u16();
            let error_text = http_response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read response text".to_string());

            tracing::error!(
                status = status,
                error = error_text,
                "Failed to make completion request to Mamba",
            );
            bail!("Failed to make completion request to Mamba: {}", error_text);
        }
        let response = http_response.json::<ChatResponse>().await?;
        //convert to ChatCompletionResponse
        Ok(response.into())
    }
}

impl From<OaiChatCompletionRequest> for ChatRequest {
    fn from(value: OaiChatCompletionRequest) -> Self {
        let mut request_builder = ChatRequest::default();
        request_builder.model = value.model;
        request_builder.messages = value
            .messages
            .into_iter()
            .map(|msg| ChatMessage {
                role: msg.role,
                content: msg.content,
            })
            .collect();
        request_builder.max_tokens = value.max_tokens;
        request_builder.temperature = value.temperature;
        request_builder.top_p = value.top_p;
        request_builder.stop = value.stop;
        request_builder.n = value.n;
        request_builder.frequency_penalty = value.frequency_penalty;
        request_builder.presence_penalty = value.presence_penalty;
        request_builder.stream = value.stream;

        request_builder
    }
}

impl From<ChatResponse> for OaiChatCompletionResponse {
    fn from(value: ChatResponse) -> Self {
        let mut response_builder = OaiChatCompletionResponseBuilder::default();
        response_builder.id(value.id).choices(
            value
                .choices
                .into_iter()
                .map(|choice| OaiChoice {
                    index: choice.index,
                    message: OaiMessage {
                        role: choice.message.role,
                        content: choice.message.content,
                    },
                    finish_reason: choice.finish_reason,
                    logprobs: None,
                })
                .collect::<Vec<OaiChoice>>(),
        );

        // Append usage if present
        if let Some(usage) = value.usage {
            response_builder.usage(OaiUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            });
        }

        response_builder.build().unwrap()
    }
}

// Request
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ChatRequest {
    model: String,

    messages: Vec<ChatMessage>,

    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

// Response

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChatChoice {
    index: u32,
    message: ChatMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChatResponse {
    id: String,
    choices: Vec<ChatChoice>,
    usage: Option<Usage>,

    // Collect unknown fields into this map
    #[serde(flatten)]
    additional_fields: HashMap<String, serde_json::Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}
