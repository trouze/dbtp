use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::core::error::Result;
use crate::core::rest_client::RestClient;

pub async fn list(
    client: &RestClient,
    params: &[(String, String)],
) -> Result<Vec<Value>> {
    client.paginate_v3("audit-logs/", params, None).await
}

/// Paginate all audit log events at or after `since`, filtering client-side.
pub async fn export_since(
    client: &RestClient,
    since: &DateTime<Utc>,
) -> Result<Vec<Value>> {
    let events = client.paginate_v3("audit-logs/", &[], None).await?;

    let filtered = events
        .into_iter()
        .filter(|e| {
            e["created_at"]
                .as_str()
                .and_then(|s| s.parse::<DateTime<Utc>>().ok())
                .map_or(true, |dt| dt >= *since)
        })
        .collect();

    Ok(filtered)
}
