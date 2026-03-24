use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::core::error::Result;
use crate::core::rest_client::RestClient;

pub async fn list(client: &RestClient, params: &[(String, String)]) -> Result<Vec<Value>> {
    client.paginate_v3("audit-logs/", params, None).await
}

/// Paginate all audit log events at or after `since`, filtering client-side.
pub async fn export_since(client: &RestClient, since: &DateTime<Utc>) -> Result<Vec<Value>> {
    let events = client.paginate_v3("audit-logs/", &[], None).await?;

    let filtered = events
        .into_iter()
        .filter(|e| event_at_or_after(e, since))
        .collect();

    Ok(filtered)
}

/// Returns true if the event has no parseable `created_at`, or if it is >= `since`.
fn event_at_or_after(event: &Value, since: &DateTime<Utc>) -> bool {
    event["created_at"]
        .as_str()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .is_none_or(|dt| dt >= *since)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ts(s: &str) -> DateTime<Utc> {
        s.parse::<DateTime<Utc>>().unwrap()
    }

    #[test]
    fn event_after_cutoff_is_included() {
        let since = ts("2024-01-10T00:00:00Z");
        let event = json!({ "created_at": "2024-01-15T12:00:00Z" });
        assert!(event_at_or_after(&event, &since));
    }

    #[test]
    fn event_at_cutoff_is_included() {
        let since = ts("2024-01-10T00:00:00Z");
        let event = json!({ "created_at": "2024-01-10T00:00:00Z" });
        assert!(event_at_or_after(&event, &since));
    }

    #[test]
    fn event_before_cutoff_is_excluded() {
        let since = ts("2024-01-10T00:00:00Z");
        let event = json!({ "created_at": "2024-01-05T00:00:00Z" });
        assert!(!event_at_or_after(&event, &since));
    }

    #[test]
    fn event_with_no_created_at_is_included() {
        let since = ts("2024-01-10T00:00:00Z");
        let event = json!({ "type": "login" });
        assert!(event_at_or_after(&event, &since));
    }

    #[test]
    fn event_with_unparseable_created_at_is_included() {
        let since = ts("2024-01-10T00:00:00Z");
        let event = json!({ "created_at": "not-a-date" });
        assert!(event_at_or_after(&event, &since));
    }
}
