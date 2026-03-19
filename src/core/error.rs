use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum DbtpError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("GraphQL error: {0}")]
    GraphQL(String),

    #[error("API error (HTTP {status}): {message}")]
    Api { status: u16, message: String },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Arrow error: {0}")]
    Arrow(String),

    #[error("Impact found")]
    ImpactFound(serde_json::Value),
}

pub type Result<T> = std::result::Result<T, DbtpError>;

impl DbtpError {
    pub fn config(msg: impl fmt::Display) -> Self {
        Self::Config(msg.to_string())
    }

    pub fn api(status: u16, msg: impl fmt::Display) -> Self {
        Self::Api {
            status,
            message: msg.to_string(),
        }
    }

    pub fn graphql(msg: impl fmt::Display) -> Self {
        Self::GraphQL(msg.to_string())
    }
}
