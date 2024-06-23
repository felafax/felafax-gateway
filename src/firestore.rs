use anyhow::{anyhow, Result};
use firestore::*;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const CUSTOMER_METADTA_COLLECTION_NAME: &'static str = "customers";

pub struct Firestore {
    project_id: String,
    db: OnceCell<FirestoreDb>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomerLLMConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomerConfig {
    pub selected_llm_name: String,
    pub selected_llm_model: String,
    pub llm_configs: HashMap<String, CustomerLLMConfig>,
}

impl Firestore {
    pub fn new(project_id: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            db: OnceCell::new(),
        }
    }

    pub fn get_project_id(&self) -> String {
        self.project_id.clone()
    }

    pub async fn get_client(&self) -> Result<firestore::FirestoreDb> {
        let db = FirestoreDb::new(&self.project_id).await?;
        Ok(db)
    }

    pub async fn get_customer_configs(&self, document_id: &str) -> Result<Option<CustomerConfig>> {
        let doc: Option<CustomerConfig> = self
            .get_client()
            .await?
            .fluent()
            .select()
            .by_id_in(CUSTOMER_METADTA_COLLECTION_NAME)
            .obj()
            .one(document_id)
            .await?;
        Ok(doc)
    }

    pub async fn list_all_collections(&self) -> Result<Vec<String>> {
        let doc = self
            .get_client()
            .await?
            .fluent()
            .list()
            .collections()
            .get_page()
            .await?
            .collection_ids;
        Ok(doc)
    }
}
