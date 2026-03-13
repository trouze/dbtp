use serde_json::Value;

use crate::core::error::Result;
use crate::core::rest_client::RestClient;

pub async fn list(client: &RestClient, run_id: u64) -> Result<Value> {
    client
        .get_v2(&format!("runs/{run_id}/artifacts/"), &[])
        .await
}

pub async fn get(client: &RestClient, run_id: u64, path: &str) -> Result<Value> {
    client
        .get_v2(&format!("runs/{run_id}/artifacts/{path}"), &[])
        .await
}
