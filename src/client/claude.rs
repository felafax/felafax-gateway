use super::traits::ChatTrait;
use crate::types::LLMConfig;
use crate::types::*;
use anyhow::bail;
use anyhow::Result;
use derive_builder::Builder;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::io::StreamReader;
use uuid::Uuid;

fn convert_err(err: reqwest::Error) -> std::io::Error {
    let err_msg = err.to_string();
    return std::io::Error::new(std::io::ErrorKind::Interrupted, err_msg);
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Model {
    id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModelListResponse {
    data: Vec<Model>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MessageRequest {
    role: String,
    content: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ClaudeCompletionRequest {
    model: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,

    messages: Vec<MessageRequest>,

    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Healthcheck {
    message: String,
}

//#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
//struct ClaudeCompletionDeltaResponse {
//    #[serde(rename = "type")]
//    _type: String,
//    text: String,
//}
//
//#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
//struct ClaudeCompletionResponse {
//    #[serde(rename = "type")]
//    _type: String,
//    delta: ClaudeCompletionDeltaResponse,
//}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeCompletionResponse {
    pub id: String,
    #[serde(rename = "type")]
    _type: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use: Option<Value>, // use Value to capture dynamic fields if needed
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

pub struct Claude {
    api_key: String,
}

impl LLMConfig for Claude {
    fn get_api_key(&self) -> String {
        self.api_key.clone()
    }

    fn set_api_key(&mut self, api_key: &str) {
        self.api_key = api_key.to_string();
    }

    fn get_base_url(&self) -> String {
        "https://api.anthropic.com".to_string()
    }

    fn get_name(&self) -> String {
        "Claude".to_string()
    }

    fn get_default_model(&self) -> String {
        "claude-3-5-sonnet-20240620".to_string()
    }
}

impl Claude {
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

impl ChatTrait for Claude {
    async fn chat(&self, request: OaiChatCompletionRequest) -> Result<OaiChatCompletionResponse> {
        //convert request to Claude request
        let mut claude_request: ClaudeCompletionRequest = request.into();
        claude_request.model = self.get_default_model();

        println!("CLAUDE REQUEST: {:?}", claude_request);

        let http_response = reqwest::Client::new()
            .post(format!("{url}/v1/messages", url = self.get_base_url()))
            .header("x-api-key", &self.get_api_key())
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "messages-2023-12-15")
            .json(&claude_request)
            .send()
            .await?;

        if !http_response.status().is_success() {
            let status = http_response.status().as_u16();
            let error_text = http_response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read response text".to_string());

            println!("CLAUDE ERROR: {:?}", error_text);
            tracing::error!(
                status = status,
                error = error_text.as_str(),
                "Failed to make completion request to Claude"
            );
            bail!(
                "Failed to make completion request to Claude: {}",
                error_text
            );
        }
        let result = http_response.json::<ClaudeCompletionResponse>().await?;
        //convert to ChatCompletionResponse
        Ok(result.into())
    }
}

impl From<OaiChatCompletionRequest> for ClaudeCompletionRequest {
    fn from(value: OaiChatCompletionRequest) -> Self {
        let mut request_builder = ClaudeCompletionRequest::default();

        //TODO: take users model
        request_builder.model = value.model;

        // TODO: Claude expects max_tokens always
        request_builder.max_tokens = Some(value.max_tokens.unwrap_or(4096));
        request_builder.messages = value
            .messages
            .into_iter()
            .map(|msg| MessageRequest {
                role: msg.role,
                content: msg.content,
            })
            .collect();
        request_builder.stream = value.stream;

        request_builder
    }
}

impl From<ClaudeCompletionResponse> for OaiChatCompletionResponse {
    fn from(value: ClaudeCompletionResponse) -> Self {
        let choices: Vec<OaiChoice> = value
            .content
            .into_iter()
            .enumerate()
            .map(|(index, block)| {
                let content = block.text.unwrap_or_default(); // Assuming text content is what you need for Message
                OaiChoice {
                    index: index as u32,
                    message: OaiMessage {
                        role: value.role.clone(),
                        content,
                    },
                    logprobs: None, // Set to None or map accordingly if you have Logprobs
                    finish_reason: value.stop_reason.clone(), // You can use stop_reason or stop_sequence
                }
            })
            .collect();

        OaiChatCompletionResponseBuilder::default()
            .id(Uuid::new_v4().to_string())
            .model(value.model)
            .created(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            )
            .choices(choices)
            .usage(OaiUsage {
                prompt_tokens: value.usage.input_tokens,
                completion_tokens: value.usage.output_tokens,
                total_tokens: value.usage.input_tokens + value.usage.output_tokens,
            })
            .build()
            .unwrap()
    }
}
