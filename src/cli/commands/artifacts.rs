use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::admin::{self, artifacts};
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct ArtifactsArgs {
    #[command(subcommand)]
    pub command: ArtifactsCommand,
}

#[derive(Debug, Subcommand)]
pub enum ArtifactsCommand {
    /// List artifacts for a run
    List { run_id: u64 },
    /// Get a specific artifact
    Get {
        run_id: u64,
        /// Artifact path (e.g. manifest.json, run_results.json)
        path: String,
    },
}

pub async fn exec(args: &ArtifactsArgs, client: &RestClient, config: &Config) -> Result<Value> {
    let is_compact = config.output == "compact";

    match &args.command {
        ArtifactsCommand::List { run_id } => {
            let val = artifacts::list(client, *run_id).await?;
            Ok(if is_compact {
                admin::compact_artifacts(&val)
            } else {
                val
            })
        }
        ArtifactsCommand::Get { run_id, path } => artifacts::get(client, *run_id, path).await,
    }
}
