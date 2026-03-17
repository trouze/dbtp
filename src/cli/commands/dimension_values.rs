use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::semantic_layer;
use crate::core::config::Config;
use crate::core::error::{DbtpError, Result};
use crate::core::graphql_client::GraphqlClient;

#[derive(Debug, Args)]
pub struct DimensionValuesArgs {
    #[command(subcommand)]
    pub command: DimensionValuesCommand,
}

#[derive(Debug, Subcommand)]
pub enum DimensionValuesCommand {
    /// List dimension values for given metrics and dimensions
    List {
        /// Metrics to query dimension values for (comma-separated or repeated)
        #[arg(long, required = true, value_delimiter = ',')]
        metrics: Vec<String>,
        /// Dimensions to retrieve values for (comma-separated or repeated)
        #[arg(long, required = true, value_delimiter = ',')]
        group_by: Vec<String>,
    },
}

pub async fn exec(
    args: &DimensionValuesArgs,
    client: &GraphqlClient,
    config: &Config,
) -> Result<Value> {
    let env_id = config.environment_id_u64().ok_or_else(|| {
        DbtpError::config(
            "environment_id is required for Semantic Layer commands. \
             Set via --environment-id, DBTP_ENVIRONMENT_ID, or config profile.",
        )
    })?;
    let host = semantic_layer::semantic_layer_url(&config.host);

    match &args.command {
        DimensionValuesCommand::List { metrics, group_by } => {
            semantic_layer::queries::list_dimension_values(
                client, &host, env_id, metrics, group_by,
            )
            .await
        }
    }
}
