use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::api::admin::environments;
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct EnvironmentsArgs {
    /// Project ID (required for all environment operations)
    #[arg(long)]
    pub project_id: u64,

    #[command(subcommand)]
    pub command: EnvironmentsCommand,
}

#[derive(Debug, Subcommand)]
pub enum EnvironmentsCommand {
    /// List environments in a project
    List {
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Show environment details
    Show { id: u64 },
    /// Create a new environment
    Create {
        #[arg(long)]
        name: String,
        /// Environment type
        #[arg(long, value_parser = ["deployment", "development"])]
        r#type: String,
    },
    /// Update an environment
    Update {
        id: u64,
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete an environment
    Delete { id: u64 },
}

pub async fn exec(
    args: &EnvironmentsArgs,
    client: &RestClient,
    _config: &Config,
) -> Result<Value> {
    let pid = args.project_id;

    match &args.command {
        EnvironmentsCommand::List { limit } => {
            let results = environments::list(client, pid, &[], *limit).await?;
            Ok(Value::Array(results))
        }
        EnvironmentsCommand::Show { id } => environments::get(client, pid, *id).await,
        EnvironmentsCommand::Create { name, r#type } => {
            let body = json!({
                "name": name,
                "type": r#type,
                "project_id": pid,
            });
            environments::create(client, pid, &body).await
        }
        EnvironmentsCommand::Update { id, name } => {
            let mut body = json!({});
            if let Some(n) = name {
                body["name"] = json!(n);
            }
            environments::update(client, pid, *id, &body).await
        }
        EnvironmentsCommand::Delete { id } => environments::delete(client, pid, *id).await,
    }
}
