use serde::{Deserialize, Serialize};

/// The standard dbt Cloud API response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub data: T,
    pub status: ResponseStatus,
    #[serde(default)]
    pub extra: Option<Extra>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseStatus {
    pub is_success: bool,
    pub code: u16,
    pub user_message: String,
    #[serde(default)]
    pub developer_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extra {
    #[serde(default)]
    pub pagination: Option<Pagination>,
    #[serde(default)]
    pub filters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub count: u64,
    pub total_count: u64,
    #[serde(default)]
    pub offset: Option<u64>,
    #[serde(default)]
    pub limit: Option<u64>,
}
