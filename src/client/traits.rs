use crate::types::*;
use anyhow::Result;

pub trait ChatTrait {
    async fn chat(&self, request: OaiChatCompletionRequest) -> Result<OaiChatCompletionResponse>;
}
