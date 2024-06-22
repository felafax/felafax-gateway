use super::traits::ChatTrait;
use crate::types::config::LLMConfig;
use crate::types::*;
use anyhow::Result;
use async_openai;

pub struct OpenAI {
    api_key: String,
}

impl LLMConfig for OpenAI {
    fn get_api_key(&self) -> String {
        self.api_key.clone()
    }

    fn set_api_key(&mut self, api_key: &str) {
        self.api_key = api_key.to_string();
    }

    fn get_base_url(&self) -> String {
        "https://api.openai.com".to_string()
    }

    fn get_name(&self) -> String {
        "OpenAI".to_string()
    }

    fn get_default_model(&self) -> String {
        "gpt-4o".to_string()
    }
}

impl OpenAI {
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

impl ChatTrait for OpenAI {
    async fn chat(&self, request: OaiChatCompletionRequest) -> Result<OaiChatCompletionResponse> {
        let config = async_openai::config::OpenAIConfig::new().with_api_key(self.api_key.clone());
        let client = async_openai::Client::with_config(config);
        let mut openai_request: async_openai::types::CreateChatCompletionRequest = request.into();
        openai_request.model = self.get_default_model();
        let response = client.chat().create(openai_request).await?;
        Ok(response.into())
    }
}

impl From<OaiChatCompletionRequest> for async_openai::types::CreateChatCompletionRequest {
    fn from(value: OaiChatCompletionRequest) -> Self {
        let mut request_builder = async_openai::types::CreateChatCompletionRequestArgs::default();

        request_builder.model(value.model);
        let messages: Vec<async_openai::types::ChatCompletionRequestMessage> = value
            .messages
            .into_iter()
            .map(|msg| {
                match msg.role.as_str() {
                    "system" => async_openai::types::ChatCompletionRequestMessage::System(
                        async_openai::types::ChatCompletionRequestSystemMessage {
                            content: msg.content,
                            name: None, // or set appropriately
                        },
                    ),
                    "user" => async_openai::types::ChatCompletionRequestMessage::User(
                        async_openai::types::ChatCompletionRequestUserMessage {
                            content:
                                async_openai::types::ChatCompletionRequestUserMessageContent::Text(
                                    msg.content,
                                ),
                            name: None, // or set appropriately
                        },
                    ),
                    "assistant" => async_openai::types::ChatCompletionRequestMessage::Assistant(
                        async_openai::types::ChatCompletionRequestAssistantMessage {
                            content: Some(msg.content),
                            name: None, // or set appropriately
                            tool_calls: None,
                            function_call: None,
                        },
                    ),
                    "tool" => async_openai::types::ChatCompletionRequestMessage::Tool(
                        async_openai::types::ChatCompletionRequestToolMessage {
                            content: msg.content,
                            tool_call_id: "".to_string(), // or set appropriately
                        },
                    ),
                    "function" => async_openai::types::ChatCompletionRequestMessage::Function(
                        async_openai::types::ChatCompletionRequestFunctionMessage {
                            content: Some(msg.content),
                            name: "".to_string(), // or set appropriately
                        },
                    ),
                    _ => panic!("Unknown role: {}", msg.role),
                }
            })
            .collect();
        request_builder.messages(messages);

        if let Some(max_tokens) = value.max_tokens {
            request_builder.max_tokens(max_tokens);
        }
        if let Some(temperature) = value.temperature {
            request_builder.temperature(temperature);
        }
        if let Some(top_p) = value.top_p {
            request_builder.top_p(top_p);
        }
        if let Some(n) = value.n {
            request_builder.n(n as u8);
        }
        if let Some(stream) = value.stream {
            request_builder.stream(stream);
        }
        //if let Some(stream_options) = value.stream_options {
        //    request_builder
        //        .stream_options(stream_options as async_openai::types::ChatCompletionStreamOptions);
        //}
        if let Some(logprobs) = value.logprobs {
            request_builder.logprobs(logprobs);
        }
        if let Some(stop) = value.stop {
            request_builder.stop(stop);
        }
        if let Some(presence_penalty) = value.presence_penalty {
            request_builder.presence_penalty(presence_penalty);
        }
        if let Some(frequency_penalty) = value.frequency_penalty {
            request_builder.frequency_penalty(frequency_penalty);
        }
        if let Some(logit_bias) = value.logit_bias {
            request_builder.logit_bias(logit_bias);
        }
        if let Some(user) = value.user {
            request_builder.user(user.to_string());
        }
        if let Some(seed) = value.seed {
            request_builder.seed(seed);
        }
        request_builder.build().unwrap()
    }
}

impl From<async_openai::types::CreateChatCompletionResponse> for OaiChatCompletionResponse {
    fn from(response: async_openai::types::CreateChatCompletionResponse) -> Self {
        let mut choices = vec![];
        for choice in response.choices {
            let message = OaiMessage {
                role: choice.message.role.to_string(),
                content: choice.message.content.unwrap_or_default(),
            };

            let finish_reason = match choice.finish_reason {
                Some(async_openai::types::FinishReason::Stop) => Some("stop".to_string()),
                Some(async_openai::types::FinishReason::Length) => Some("length".to_string()),
                Some(async_openai::types::FinishReason::ToolCalls) => {
                    Some("tool_calls".to_string())
                }
                Some(async_openai::types::FinishReason::ContentFilter) => {
                    Some("content_filter".to_string())
                }
                Some(async_openai::types::FinishReason::FunctionCall) => {
                    Some("function_call".to_string())
                }
                _ => None,
            };
            // TODO: add logpobs
            choices.push(OaiChoice {
                index: choice.index,
                message,
                //finish_reason: choice.finish_reason.unwrap_or(None),
                finish_reason,
                logprobs: None,
            });
        }
        let usage = match response.usage {
            Some(usage) => Some(OaiUsage {
                prompt_tokens: usage.prompt_tokens.into(),
                completion_tokens: usage.completion_tokens.into(),
                total_tokens: usage.total_tokens.into(),
            }),
            None => None,
        };
        OaiChatCompletionResponse {
            id: response.id,
            object: response.object,
            created: response.created.into(),
            model: response.model,
            system_fingerprint: response.system_fingerprint,
            choices,
            usage,
        }
    }
}
