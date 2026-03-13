mod api;
mod cli;
mod core;

use clap::Parser;

use crate::cli::Cli;
use crate::core::config::{self, ConfigOverrides};
use crate::core::graphql_client::GraphqlClient;
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
        profile: cli.global.profile.clone(),
        host: cli.global.host.clone(),
        token: cli.global.token.clone(),
        account_id: cli.global.account_id,
        environment_id: cli.global.environment_id,
        output: cli.global.output.clone(),
    };

    let config = config::load(overrides)?;

    let rest = RestClient::new(&config.host, &config.token, config.account_id)?;
    let gql = GraphqlClient::new(&config.token)?;

    cli::commands::exec(&cli.command, &rest, &gql, &config).await
}
