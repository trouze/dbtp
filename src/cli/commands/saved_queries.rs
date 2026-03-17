use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::semantic_layer;
use crate::core::config::Config;
use crate::core::error::{DbtpError, Result};
use crate::core::graphql_client::GraphqlClient;

#[derive(Debug, Args)]
pub struct SavedQueriesArgs {
    #[command(subcommand)]
    pub command: SavedQueriesCommand,
}

#[derive(Debug, Subcommand)]
pub enum SavedQueriesCommand {
    /// List saved queries
    List {
        /// Search filter
        #[arg(long)]
        search: Option<String>,
    },
}

pub async fn exec(
    args: &SavedQueriesArgs,
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
        SavedQueriesCommand::List { search } => {
            semantic_layer::queries::list_saved_queries(client, &host, env_id, search.as_deref())
                .await
        }
    }
}
