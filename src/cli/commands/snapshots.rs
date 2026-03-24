use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::discovery::resource_details::ResourceType;
use crate::api::discovery::{require_environment_id, resource_details};
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

#[derive(Debug, Args)]
pub struct SnapshotsArgs {
    #[command(subcommand)]
    pub command: SnapshotsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SnapshotsCommand {
    /// Show detailed snapshot information
    Show {
        /// Snapshot name or unique_id
        identifier: String,
    },
}

pub async fn exec(args: &SnapshotsArgs, client: &GraphqlClient, config: &Config) -> Result<Value> {
    let env_id = require_environment_id(config)?;

    match &args.command {
        SnapshotsCommand::Show { identifier } => {
            resource_details::fetch_details(
                client,
                &config.host,
                env_id,
                ResourceType::Snapshot,
                identifier,
            )
            .await
        }
    }
}
