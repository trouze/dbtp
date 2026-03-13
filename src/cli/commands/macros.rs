use clap::{Args, Subcommand};
use serde_json::Value;

use crate::api::discovery::{macros, require_environment_id, resource_details};
use crate::api::discovery::resource_details::ResourceType;
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

#[derive(Debug, Args)]
pub struct MacrosArgs {
    #[command(subcommand)]
    pub command: MacrosCommand,
}

#[derive(Debug, Subcommand)]
pub enum MacrosCommand {
    /// List macros in the environment
    List {
        /// Filter to a specific package name
        #[arg(long)]
        package: Option<String>,

        /// Only return unique package names (not full macro details)
        #[arg(long)]
        packages_only: bool,

        /// Include dbt built-in packages (dbt, dbt_bigquery, etc.)
        #[arg(long)]
        include_dbt_packages: bool,
    },
    /// Show detailed macro information
    Show {
        /// Macro name or unique_id
        identifier: String,
    },
}

pub async fn exec(
    args: &MacrosArgs,
    client: &GraphqlClient,
    config: &Config,
) -> Result<Value> {
    let env_id = require_environment_id(config)?;

    match &args.command {
        MacrosCommand::List {
            package,
            packages_only,
            include_dbt_packages,
        } => {
            macros::list(
                client,
                &config.host,
                env_id,
                package.as_deref(),
                *packages_only,
                *include_dbt_packages,
            )
            .await
        }
        MacrosCommand::Show { identifier } => {
            resource_details::fetch_details(
                client,
                &config.host,
                env_id,
                ResourceType::Macro,
                identifier,
            )
            .await
        }
    }
}
