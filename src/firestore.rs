use anyhow::{anyhow, Result};
use firestore::*;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const METADTA_COLLECTION_NAME: &'static str = "configs";

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

    pub async fn init(&self) -> Result<()> {
        //let db = FirestoreDb::new(&self.project_id).await?;
        let db = FirestoreDb::with_options_service_account_key_file(
            FirestoreDbOptions::new(self.get_project_id()),
            "firebase.json".into(),
        )
        .await?;
        self.db.set(db.clone()).unwrap();
        Ok(())
    }

    pub fn get_client(&self) -> &firestore::FirestoreDb {
        self.db.get().unwrap()
    }

    pub async fn get_customer_configs(&self, document_id: &str) -> Result<Option<CustomerConfig>> {
        let doc: Option<CustomerConfig> = self
            .get_client()
            .fluent()
            .select()
            .by_id_in(METADTA_COLLECTION_NAME)
            .obj()
            .one(document_id)
            .await?;
        Ok(doc)
    }

    pub async fn list_all_collections(&self) -> Result<Vec<String>> {
        let doc = self
            .get_client()
            .fluent()
            .list()
            .collections()
            .get_page()
            .await?
            .collection_ids;
        Ok(doc)
    }
}
