use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::discovery::resource_details::ResourceType;
use crate::api::discovery::{require_environment_id, resource_details, sources};
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

#[derive(Debug, Args)]
pub struct SourcesArgs {
    #[command(subcommand)]
    pub command: SourcesCommand,
}

#[derive(Debug, Subcommand)]
pub enum SourcesCommand {
    /// List sources in the environment
    List {
        /// Filter by source name (e.g. "raw_payments")
        #[arg(long)]
        source_name: Option<String>,
    },
    /// Show detailed source information
    Show {
        /// Source name or unique_id
        identifier: String,
    },
}

pub async fn exec(args: &SourcesArgs, client: &GraphqlClient, config: &Config) -> Result<Value> {
    let env_id = require_environment_id(config)?;

    match &args.command {
        SourcesCommand::List { source_name } => {
            let nodes = sources::list(client, &config.host, env_id, source_name.as_deref()).await?;
            Ok(Value::Array(nodes))
        }
        SourcesCommand::Show { identifier } => {
            resource_details::fetch_details(
                client,
                &config.host,
                env_id,
                ResourceType::Source,
                identifier,
            )
            .await
        }
    }
}
