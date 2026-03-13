use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::discovery::{models, resource_details};
use crate::api::discovery::resource_details::ResourceType;
use crate::api::discovery::require_environment_id;
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

#[derive(Debug, Args)]
pub struct ModelsArgs {
    #[command(subcommand)]
    pub command: ModelsCommand,
}

#[derive(Debug, Subcommand)]
pub enum ModelsCommand {
    /// List models in the environment
    List {
        /// Only show mart-layer models
        #[arg(long)]
        mart_only: bool,
    },
    /// Show detailed model information
    Show {
        /// Model name or unique_id (e.g. "orders" or "model.analytics.orders")
        identifier: String,
    },
    /// Show a model's parent dependencies
    Parents {
        /// Model name or unique_id
        identifier: String,
    },
    /// Show a model's child dependents
    Children {
        /// Model name or unique_id
        identifier: String,
    },
    /// Show model health: execution status, tests, and ancestor health
    Health {
        /// Model name or unique_id
        identifier: String,
    },
    /// Show model execution performance over recent runs
    Performance {
        /// Model name or unique_id
        identifier: String,

        /// Number of historical runs to retrieve
        #[arg(long, default_value = "10")]
        num_runs: u32,

        /// Include test execution data for each run
        #[arg(long)]
        include_tests: bool,
    },
}

pub async fn exec(
    args: &ModelsArgs,
    client: &GraphqlClient,
    config: &Config,
) -> Result<Value> {
    let env_id = require_environment_id(config)?;

    match &args.command {
        ModelsCommand::List { mart_only } => {
            let nodes = models::list(client, &config.host, env_id, *mart_only).await?;
            Ok(Value::Array(nodes))
        }
        ModelsCommand::Show { identifier } => {
            resource_details::fetch_details(
                client,
                &config.host,
                env_id,
                ResourceType::Model,
                identifier,
            )
            .await
        }
        ModelsCommand::Parents { identifier } => {
            models::parents(client, &config.host, env_id, identifier).await
        }
        ModelsCommand::Children { identifier } => {
            models::children(client, &config.host, env_id, identifier).await
        }
        ModelsCommand::Health { identifier } => {
            models::health(client, &config.host, env_id, identifier).await
        }
        ModelsCommand::Performance {
            identifier,
            num_runs,
            include_tests,
        } => {
            let uid = resolve_to_unique_id(client, config, env_id, identifier).await?;
            models::performance(client, &config.host, env_id, &uid, *num_runs, *include_tests)
                .await
        }
    }
}

/// Performance requires a unique_id. If the user gave a name, resolve it first.
async fn resolve_to_unique_id(
    client: &GraphqlClient,
    config: &Config,
    env_id: u64,
    identifier: &str,
) -> Result<String> {
    if identifier.contains('.') {
        return Ok(identifier.to_string());
    }
    let details = resource_details::fetch_details(
        client,
        &config.host,
        env_id,
        ResourceType::Model,
        identifier,
    )
    .await?;

    match &details {
        Value::Array(arr) if arr.len() > 1 => {
            let matches: Vec<&str> = arr
                .iter()
                .filter_map(|d| d["uniqueId"].as_str())
                .collect();
            Err(crate::core::error::DbtpError::graphql(format!(
                "Multiple models found for '{}'. Provide a unique_id: {}",
                identifier,
                matches.join(", ")
            )))
        }
        Value::Array(arr) => arr
            .first()
            .and_then(|d| d["uniqueId"].as_str())
            .map(String::from)
            .ok_or_else(|| {
                crate::core::error::DbtpError::graphql(format!("Model '{}' not found", identifier))
            }),
        other => other["uniqueId"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| {
                crate::core::error::DbtpError::graphql(format!("Model '{}' not found", identifier))
            }),
    }
}
