use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::discovery::{lineage, require_environment_id};
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

#[derive(Debug, Args)]
pub struct LineageArgs {
    #[command(subcommand)]
    pub command: LineageCommand,
}

#[derive(Debug, Subcommand)]
pub enum LineageCommand {
    /// Show lineage graph for a node (upstream + downstream)
    Show {
        /// Unique ID of the resource (e.g. "model.analytics.orders")
        unique_id: String,

        /// Max hops from the target node (0 = unlimited)
        #[arg(long, default_value = "5")]
        depth: u32,

        /// Resource types to include (e.g. Model, Source, Exposure)
        #[arg(long, value_delimiter = ',')]
        types: Vec<String>,
    },
}

pub async fn exec(
    args: &LineageArgs,
    client: &GraphqlClient,
    config: &Config,
) -> Result<Value> {
    let env_id = require_environment_id(config)?;

    match &args.command {
        LineageCommand::Show {
            unique_id,
            depth,
            types,
        } => lineage::fetch(client, &config.host, env_id, unique_id, *depth, types).await,
    }
}
