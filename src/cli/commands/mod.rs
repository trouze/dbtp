pub mod accounts;
pub mod artifacts;
pub mod configure;
pub mod dimension_values;
pub mod environments;
pub mod exposures;
pub mod jobs;
pub mod lineage;
pub mod macros;
pub mod metrics;
pub mod models;
pub mod projects;
pub mod runs;
pub mod saved_queries;
pub mod seeds;
pub mod semantic_models;
pub mod snapshots;
pub mod sources;
pub mod system;
pub mod tests;

use crate::cli::output::{format_output, OutputFormat};
use crate::cli::{Cli, Commands};
use crate::core::config::Config;
use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;
use crate::core::rest_client::RestClient;

pub async fn exec(
    cmd: &Commands,
    rest: &RestClient,
    gql: &GraphqlClient,
    config: &Config,
) -> Result<()> {
    let output_format = OutputFormat::parse(&config.output);

    match cmd {
        Commands::Configure(args) => {
            configure::exec(args).await?;
            return Ok(());
        }

        Commands::Completion { shell } => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            clap_complete::generate(*shell, &mut cmd, "dbtp", &mut std::io::stdout());
            return Ok(());
        }

        Commands::System(args) => {
            system::exec(args).await?;
            return Ok(());
        }

        // Admin API commands
        Commands::Accounts(args) => {
            let val = accounts::exec(args, rest, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Projects(args) => {
            let val = projects::exec(args, rest, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Environments(args) => {
            let val = environments::exec(args, rest, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Jobs(args) => {
            let val = jobs::exec(args, rest, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Runs(args) => {
            let val = runs::exec(args, rest, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Artifacts(args) => {
            let val = artifacts::exec(args, rest, config).await?;
            println!("{}", format_output(&val, output_format));
        }

        // Discovery API commands
        Commands::Models(args) => {
            let val = models::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Lineage(args) => {
            let val = lineage::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Sources(args) => {
            let val = sources::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Exposures(args) => {
            let val = exposures::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Macros(args) => {
            let val = macros::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Seeds(args) => {
            let val = seeds::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Snapshots(args) => {
            let val = snapshots::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::Tests(args) => {
            let val = tests::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::SemanticModels(args) => {
            let val = semantic_models::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }

        // Semantic Layer API commands
        Commands::Metrics(args) => {
            let val = metrics::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::SavedQueries(args) => {
            let val = saved_queries::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
        Commands::DimensionValues(args) => {
            let val = dimension_values::exec(args, gql, config).await?;
            println!("{}", format_output(&val, output_format));
        }
    }

    Ok(())
}
