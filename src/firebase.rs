use anyhow::Result;
use firebase_rs::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const CUSTOMER_METADTA_COLLECTION_NAME: &'static str = "customers";

pub struct Firestore {
    project_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomerConfig {
    pub llm_name: String,
    pub llm_model_name: String,
    pub api_key: String,
}

impl Firestore {
    pub fn new(project_id: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
        }
    }

    pub fn get_project_id(&self) -> String {
        self.project_id.clone()
    }

    pub async fn get_client(&self) -> Result<firebase_rs::Firebase> {
        //let db = FirestoreDb::new(&self.project_id).await?;
        let firebase = Firebase::new(&self.project_id).unwrap();
        Ok(firebase)
    }

    pub async fn get_customer_configs(&self, document_id: &str) -> Result<Option<CustomerConfig>> {
        let docs = self
            .get_client()
            .await?
            .at(CUSTOMER_METADTA_COLLECTION_NAME);
        let customer_config = docs.get::<HashMap<String, CustomerConfig>>().await?;
        if !customer_config.contains_key(document_id) {
            return Ok(None);
        }
        Ok(Some(customer_config.get(document_id).unwrap().clone()))
    }
}
