use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Builder, Deserialize, Clone, PartialEq, Serialize, Default)]
#[builder(setter(into, strip_option), default)]
#[builder(pattern = "mutable")]
#[builder(derive(Debug))]
pub struct OaiChatCompletionRequest {
    pub model: String,

    pub messages: Vec<OaiMessage>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, serde_json::Value>>, // default: null
    //
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
pub struct OaiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Builder, Deserialize, Clone, PartialEq, Serialize, Default)]
#[builder(setter(into, strip_option), default)]
#[builder(pattern = "mutable")]
#[builder(derive(Debug))]
pub struct OaiChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub system_fingerprint: Option<String>,
    pub choices: Vec<OaiChoice>,
    pub usage: Option<OaiUsage>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
pub struct OaiChoice {
    pub index: u32,
    pub message: OaiMessage,
    pub logprobs: Option<OaiLogprobs>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
pub struct OaiLogprobs {
    pub tokens: Vec<String>,
    pub token_logprobs: Vec<f64>,
    pub top_logprobs: Option<Vec<HashMap<String, f64>>>,
    pub text_offset: Vec<u32>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize, Default)]
pub struct OaiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
