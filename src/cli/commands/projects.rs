use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::api::admin::{self, projects};
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::rest_client::RestClient;

#[derive(Debug, Args)]
pub struct ProjectsArgs {
    #[command(subcommand)]
    pub command: ProjectsCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProjectsCommand {
    /// List projects
    List {
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Show project details
    Show { id: u64 },
    /// Create a new project
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Update a project
    Update {
        id: u64,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a project
    Delete { id: u64 },
}

pub async fn exec(args: &ProjectsArgs, client: &RestClient, config: &Config) -> Result<Value> {
    let is_compact = config.output == "compact";
    let is_table = config.output == "table" || config.output.is_empty();

    match &args.command {
        ProjectsCommand::List { limit } => {
            let results = projects::list(client, &[], *limit).await?;
            let val = Value::Array(results);
            Ok(if is_table {
                admin::table_view(&val, admin::PROJECTS_TABLE_FIELDS)
            } else {
                val
            })
        }
        ProjectsCommand::Show { id } => {
            let val = projects::get(client, *id).await?;
            Ok(if is_compact {
                admin::compact_project(&val)
            } else {
                val
            })
        }
        ProjectsCommand::Create { name, description } => {
            let mut body = json!({ "name": name });
            if let Some(desc) = description {
                body["description"] = json!(desc);
            }
            projects::create(client, &body).await
        }
        ProjectsCommand::Update {
            id,
            name,
            description,
        } => {
            let mut body = json!({});
            if let Some(n) = name {
                body["name"] = json!(n);
            }
            if let Some(d) = description {
                body["description"] = json!(d);
            }
            projects::update(client, *id, &body).await
        }
        ProjectsCommand::Delete { id } => projects::delete(client, *id).await,
    }
}
