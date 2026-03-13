use serde_json::Value;

use crate::core::error::Result;
use crate::core::rest_client::RestClient;

pub async fn list(
    client: &RestClient,
    project_id: u64,
    params: &[(String, String)],
    limit: Option<u64>,
) -> Result<Vec<Value>> {
    client
        .paginate_v3(
            &format!("projects/{project_id}/environments/"),
            params,
            limit,
        )
        .await
}

pub async fn get(
    client: &RestClient,
    project_id: u64,
    environment_id: u64,
) -> Result<Value> {
    client
        .get_v3(
            &format!("projects/{project_id}/environments/{environment_id}/"),
            &[],
        )
        .await
}

pub async fn create(
    client: &RestClient,
    project_id: u64,
    body: &Value,
) -> Result<Value> {
    client
        .post_v3(
            &format!("projects/{project_id}/environments/"),
            body,
        )
        .await
}

pub async fn update(
    client: &RestClient,
    project_id: u64,
    environment_id: u64,
    body: &Value,
) -> Result<Value> {
    client
        .post_v3(
            &format!("projects/{project_id}/environments/{environment_id}/"),
            body,
        )
        .await
}

pub async fn delete(
    client: &RestClient,
    project_id: u64,
    environment_id: u64,
) -> Result<Value> {
    client
        .delete_v3(&format!(
            "projects/{project_id}/environments/{environment_id}/"
        ))
        .await
}
