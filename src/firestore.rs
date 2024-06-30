use crate::request_logs;
use anyhow::Result;
use firestore::*;
use futures::StreamExt;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const METADTA_COLLECTION_NAME: &'static str = "configs";
const CUSTOMER_COLLECTION_NAME: &'static str = "users";

pub struct Firestore {
    project_id: String,
    service_account_json_path: String,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FelafaxTokenToIdMap {
    pub felafax_token_to_id_map: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rollout {
    pub rollout_id: String,
    pub rollout_name: String,
    pub rollout_percentage: f64,
    pub created_date: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserRollouts {
    pub rollouts: Vec<Rollout>,
}

impl Firestore {
    pub fn new(project_id: &str, service_acccount_json_path: &str) -> Self {
        // check if service_account_key_file exists
        if !std::path::Path::new(service_acccount_json_path).exists() {
            panic!(
                "Service account key file does not exist: {}",
                service_acccount_json_path
            );
        }
        Self {
            project_id: project_id.to_string(),
            service_account_json_path: service_acccount_json_path.to_string(),
            db: OnceCell::new(),
        }
    }

    pub fn get_project_id(&self) -> String {
        self.project_id.clone()
    }

    pub async fn get_roll_out_percentage(
        &self,
        user_id: &str,
        rollout_id: &str,
    ) -> Result<Option<i64>> {
        let rollouts = self.fetch_user_rollouts(user_id).await?;
        println!("ROLLOUTS: {:?}", rollouts);
        for rollout in rollouts {
            if rollout.rollout_id == rollout_id {
                return Ok(Some(rollout.rollout_percentage as i64));
            }
        }
        Ok(None)
    }

    async fn fetch_user_rollouts(&self, user_id: &str) -> Result<Vec<Rollout>> {
        let user_rollouts: Option<UserRollouts> = self
            .get_client()
            .fluent()
            .select()
            .by_id_in("rollouts")
            .obj()
            .one(user_id)
            .await?;

        // 2. Extract rollouts or return an empty vector if not found
        match user_rollouts {
            Some(ur) => Ok(ur.rollouts),
            None => Ok(Vec::new()),
        }
    }

    //pub async fn get_roll_outs(&self, user_id: &str) -> Result<Vec<Rollout>> {
    //    const ROLLOUT_COLLECTION_NAME: &str = "rollouts";
    //    const USER_ROLLOUTS_COLLECTION_NAME: &str = "userRollouts";
    //
    //    let client = self.get_client();
    //
    //    // Construct the path for the user's rollout reference
    //    let user_rollout_ref = format!("{}/{}", ROLLOUT_COLLECTION_NAME, user_id);
    //    println!("USER_ROLLOUT_REF: {:?}", user_rollout_ref);
    //
    //    // Fetch the list of rollout IDs
    //    let rollout_docs = client
    //        .fluent()
    //        .select()
    //        .from(USER_ROLLOUTS_COLLECTION_NAME)
    //        .parent(&user_rollout_ref)
    //        .query()
    //        .await?;
    //
    //    println!("ROLLOUT_DOCS: {:?}", rollout_docs);
    //
    //    let rollout_ids: Vec<String> = rollout_docs
    //        .into_iter()
    //        .map(|doc| doc.name.to_string())
    //        .collect();
    //
    //    println!("ROLLOUT_IDS: {:?}", rollout_ids);
    //
    //    // Fetch details for each rollout
    //    let mut rollouts = Vec::new();
    //    for rollout_id in rollout_ids {
    //        if let Some(rollout_doc) = client
    //            .fluent()
    //            .select()
    //            .by_id_in(USER_ROLLOUTS_COLLECTION_NAME)
    //            .parent(&user_rollout_ref)
    //            .obj::<Rollout>()
    //            .one(&rollout_id)
    //            .await?
    //        {
    //            rollouts.push(rollout_doc);
    //        }
    //    }
    //
    //    Ok(rollouts)
    //}

    pub async fn get_user_id(&self, felafax_token: &str) -> Result<Option<String>> {
        let id_to_user_map = self.get_id_to_user_map().await?;
        println!("ID_TO_USER_MAP: {:?}", id_to_user_map);
        match id_to_user_map {
            Some(doc) => {
                if doc.felafax_token_to_id_map.contains_key(felafax_token) {
                    let user_id = doc.felafax_token_to_id_map.get(felafax_token);
                    Ok(user_id.cloned())
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn get_id_to_user_map(&self) -> Result<Option<FelafaxTokenToIdMap>> {
        let doc: Option<FelafaxTokenToIdMap> = self
            .get_client()
            .fluent()
            .select()
            .by_id_in(CUSTOMER_COLLECTION_NAME)
            .obj()
            .one("metadata")
            .await?;
        Ok(doc)
    }

    pub async fn init(&self) -> Result<()> {
        //let db = FirestoreDb::new(&self.project_id).await?;
        let db = FirestoreDb::with_options_service_account_key_file(
            FirestoreDbOptions::new(self.get_project_id()),
            self.service_account_json_path.clone().into(),
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

    pub async fn insert_request_log(&self, request_logs: &request_logs::RequestLog) -> Result<()> {
        self.get_client()
            .fluent()
            .insert()
            .into("request_logs")
            .document_id(&request_logs.id)
            .object(request_logs)
            .execute()
            .await?;
        Ok(())
    }
}
