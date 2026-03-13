use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::discovery::{exposures, require_environment_id, resource_details};
use crate::api::discovery::resource_details::ResourceType;
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

#[derive(Debug, Args)]
pub struct ExposuresArgs {
    #[command(subcommand)]
    pub command: ExposuresCommand,
}

#[derive(Debug, Subcommand)]
pub enum ExposuresCommand {
    /// List exposures in the environment
    List,
    /// Show detailed exposure information
    Show {
        /// Exposure name or unique_id
        identifier: String,
    },
}

pub async fn exec(
    args: &ExposuresArgs,
    client: &GraphqlClient,
    config: &Config,
) -> Result<Value> {
    let env_id = require_environment_id(config)?;

    match &args.command {
        ExposuresCommand::List => {
            let nodes = exposures::list(client, &config.host, env_id).await?;
            Ok(Value::Array(nodes))
        }
        ExposuresCommand::Show { identifier } => {
            resource_details::fetch_details(
                client,
                &config.host,
                env_id,
                ResourceType::Exposure,
                identifier,
            )
            .await
        }
    }
}
