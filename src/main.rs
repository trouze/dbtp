mod api;
mod cli;
mod core;

use clap::Parser;

use crate::cli::Cli;
use crate::core::config::{self, ConfigOverrides};
use crate::core::graphql_client::GraphqlClient;
use crate::core::resolve;
use crate::core::rest_client::RestClient;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> crate::core::error::Result<()> {
    let overrides = ConfigOverrides {
        host: cli.global.host.clone(),
        token: cli.global.token.clone(),
        service_token: cli.global.service_token.clone(),
        account_id: cli.global.account_id,
        project_id: cli.global.project_id.clone(),
        environment_id: cli.global.environment_id.clone(),
        output: cli.global.output.clone(),
    };

    let mut config = config::load(overrides)?;

    let rest = RestClient::new(&config.host, &config.token, config.account_id)?;
    let gql = GraphqlClient::new(&config.token)?;

    if let Some(ref raw) = config.project_id {
        let resolved = resolve::resolve_project(&rest, raw).await?;
        config.project_id = Some(resolved.to_string());
    }
    if let Some(ref raw) = config.environment_id {
        let resolved = resolve::resolve_environment(&rest, config.project_id_u64(), raw).await?;
        config.environment_id = Some(resolved.to_string());
    }

    if config.environment_id.is_none() {
        if let Some(pid) = config.project_id_u64() {
            if let Ok(env_id) = resolve::resolve_production_environment(&rest, pid).await {
                config.environment_id = Some(env_id.to_string());
            }
        }
    }

    cli::commands::exec(&cli.command, &rest, &gql, &config).await
}
