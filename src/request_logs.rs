use crate::clickhouse as cl;
use crate::firestore;
use anyhow::Result;
use clickhouse::Row;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Builder, Deserialize, Clone, PartialEq, Serialize, Row, Default)]
#[builder(setter(into, strip_option), default)]
#[builder(pattern = "mutable")]
#[builder(derive(Debug))]
#[serde(default)]
pub struct RequestLog {
    pub request_id: String,

    pub customer_id: String,

    pub timestamp: i64,

    pub felafax_token: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_name: Option<String>,

    pub llm_model: String,

    pub http_status: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,

    pub total_latency: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RequestLog {
    pub async fn log(
        &self,
        client: &Arc<cl::Clickhouse>,
        firestore: &Arc<firestore::Firestore>,
    ) -> Result<()> {
        println!("Logging request: {:?}", self);
        // TODO: move ot clickhouse or postgres
        // client.insert_row("request_logs", self.clone()).await?;

        if self.customer_id.is_empty() {
            eprintln!("Customer ID is empty, skipping logging");
        }

        firestore
            .insert_request_log(&self.clone())
            .await
            .unwrap_or_else(|e| eprintln!("Failed to log request: {:?}", e));
        Ok(())
    }
}
