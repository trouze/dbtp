pub mod commands;
pub mod output;

use clap::{Parser, Subcommand};

use self::commands::{
    accounts, artifacts, configure, dimension_values, environments, exposures, jobs, lineage,
    macros, metrics, models, projects, runs, saved_queries, seeds, semantic_models, snapshots,
    sources, system, tests,
};

#[derive(Debug, Parser)]
#[command(
    name = "dbtp",
    version,
    propagate_version = true,
    about = "CLI for the dbt Cloud Platform API",
    long_about = "CLI for the dbt Cloud Platform API.\n\n\
        dbtp gives you terminal access to dbt Cloud's Admin, Discovery, and Semantic Layer APIs.\n\
        Manage accounts, projects, environments, jobs, and runs. Browse your DAG metadata.\n\
        Query metrics. All output formats (table, json, yaml) are supported.\n\n\
        Get started:\n  \
        dbtp configure          Set up credentials\n  \
        dbtp accounts list      Verify access\n  \
        dbtp projects list      List your projects",
    after_long_help = "EXAMPLES:\n  \
        dbtp jobs trigger 48213 --cause \"nightly refresh\"\n  \
        dbtp runs wait 901244 --interval 15\n  \
        dbtp models health orders --environment-id 67890\n  \
        dbtp metrics query revenue --group-by metric_time --grain MONTH\n  \
        dbtp artifacts get 901244 manifest.json -o json"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOpts,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Parser)]
pub struct GlobalOpts {
    /// Output format: table, json, yaml, compact
    #[arg(long, short, global = true, default_value = "table")]
    pub output: Option<String>,

    /// Named config profile to use
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// dbt Cloud host URL
    #[arg(long, global = true, env = "DBTP_HOST")]
    pub host: Option<String>,

    /// dbt Cloud API token
    #[arg(long, global = true, env = "DBTP_TOKEN")]
    pub token: Option<String>,

    /// dbt Cloud account ID
    #[arg(long, global = true, env = "DBTP_ACCOUNT_ID")]
    pub account_id: Option<u64>,

    /// dbt Cloud environment ID (for Discovery / Semantic Layer)
    #[arg(long, global = true, env = "DBTP_ENVIRONMENT_ID")]
    pub environment_id: Option<u64>,

    /// Enable verbose output
    #[arg(long, short, global = true)]
    pub verbose: bool,

    /// JMESPath query to filter JSON output
    #[arg(long, global = true)]
    pub query: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Set up CLI profiles and credentials
    #[command(
        long_about = "Set up CLI profiles and credentials.\n\n\
            Interactively configure a named profile with your dbt Cloud host, API token,\n\
            account ID, and environment ID. Profiles are saved to ~/.config/dbtp/config.toml.",
        after_long_help = "EXAMPLES:\n  \
            dbtp configure                      # Configure the 'default' profile\n  \
            dbtp configure --profile-name prod  # Configure a 'prod' profile"
    )]
    Configure(configure::ConfigureArgs),

    // ── Admin API ──────────────────────────────────────
    /// Manage dbt Cloud accounts
    #[command(
        long_about = "Manage dbt Cloud accounts.\n\n\
            List all accounts you have access to, or show details for a specific account.",
        after_long_help = "EXAMPLES:\n  \
            dbtp accounts list\n  \
            dbtp accounts show\n  \
            dbtp accounts show 51798"
    )]
    Accounts(accounts::AccountsArgs),

    /// Manage dbt Cloud projects
    #[command(
        long_about = "Manage dbt Cloud projects.\n\n\
            List, create, update, and delete projects within your account.",
        after_long_help = "EXAMPLES:\n  \
            dbtp projects list\n  \
            dbtp projects show 12345\n  \
            dbtp projects create --name \"Analytics\" --description \"Main analytics project\"\n  \
            dbtp projects update 12345 --name \"Analytics v2\"\n  \
            dbtp projects delete 12345"
    )]
    Projects(projects::ProjectsArgs),

    /// Manage dbt Cloud environments
    #[command(
        long_about = "Manage dbt Cloud environments within a project.\n\n\
            List, create, update, and delete environments. Requires --project-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp environments --project-id 123 list\n  \
            dbtp environments --project-id 123 show 456\n  \
            dbtp environments --project-id 123 create --name staging --type deployment\n  \
            dbtp environments --project-id 123 delete 456"
    )]
    Environments(environments::EnvironmentsArgs),

    /// Manage dbt Cloud jobs
    #[command(
        long_about = "Manage dbt Cloud jobs.\n\n\
            List, create, update, delete, and trigger jobs. Use 'trigger' to start a run\n\
            and 'trigger-from-failure' to rerun from the point of failure.",
        after_long_help = "EXAMPLES:\n  \
            dbtp jobs list --project-id 123\n  \
            dbtp jobs show 48213\n  \
            dbtp jobs trigger 48213 --cause \"deploy from CI\"\n  \
            dbtp jobs trigger 48213 --git-branch feature/new-models\n  \
            dbtp jobs trigger-from-failure 48213\n  \
            dbtp jobs create --name \"Nightly\" --project-id 123 --environment-id 456"
    )]
    Jobs(jobs::JobsArgs),

    /// Manage dbt Cloud runs
    #[command(
        long_about = "Manage dbt Cloud runs.\n\n\
            List, inspect, cancel, and retry runs. Use 'wait' to poll until a run\n\
            reaches a terminal state. Use 'errors' to extract failure details.",
        after_long_help = "EXAMPLES:\n  \
            dbtp runs list --job-id 48213 --status running\n  \
            dbtp runs show 901244\n  \
            dbtp runs wait 901244 --interval 10 --timeout 1800\n  \
            dbtp runs errors 901244\n  \
            dbtp runs cancel 901244\n  \
            dbtp runs retry 901244"
    )]
    Runs(runs::RunsArgs),

    /// Download dbt Cloud run artifacts
    #[command(
        long_about = "Download dbt Cloud run artifacts.\n\n\
            List available artifacts for a completed run, or download a specific file\n\
            such as manifest.json or run_results.json.",
        after_long_help = "EXAMPLES:\n  \
            dbtp artifacts list 901244\n  \
            dbtp artifacts get 901244 manifest.json\n  \
            dbtp artifacts get 901244 run_results.json -o json"
    )]
    Artifacts(artifacts::ArtifactsArgs),

    // ── Discovery API ──────────────────────────────────
    /// Browse dbt models via the Discovery API
    #[command(
        long_about = "Browse dbt models via the Discovery API.\n\n\
            List models, inspect details, explore lineage (parents/children),\n\
            check health, and view execution performance. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp models list --environment-id 67890\n  \
            dbtp models list --mart-only\n  \
            dbtp models show orders\n  \
            dbtp models parents orders\n  \
            dbtp models children stg_payments\n  \
            dbtp models health orders\n  \
            dbtp models performance orders --num-runs 20 --include-tests"
    )]
    Models(models::ModelsArgs),

    /// Explore model lineage via the Discovery API
    #[command(
        long_about = "Explore model lineage via the Discovery API.\n\n\
            Show the upstream and downstream dependency graph for any dbt node.\n\
            Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp lineage show model.analytics.orders\n  \
            dbtp lineage show model.analytics.orders --depth 3\n  \
            dbtp lineage show model.analytics.orders --types Model,Source"
    )]
    Lineage(lineage::LineageArgs),

    /// Browse dbt sources via the Discovery API
    #[command(
        long_about = "Browse dbt sources via the Discovery API.\n\n\
            List and inspect source definitions. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp sources list\n  \
            dbtp sources list --source-name raw_payments\n  \
            dbtp sources show raw_payments"
    )]
    Sources(sources::SourcesArgs),

    /// Browse dbt exposures via the Discovery API
    #[command(
        long_about = "Browse dbt exposures via the Discovery API.\n\n\
            List and inspect exposure definitions. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp exposures list\n  \
            dbtp exposures show weekly_revenue_dashboard"
    )]
    Exposures(exposures::ExposuresArgs),

    /// Browse dbt macros via the Discovery API
    #[command(
        long_about = "Browse dbt macros via the Discovery API.\n\n\
            List macros by package, list unique package names, or show details\n\
            for a specific macro. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp macros list\n  \
            dbtp macros list --packages-only\n  \
            dbtp macros list --package dbt_utils\n  \
            dbtp macros show generate_surrogate_key"
    )]
    Macros(macros::MacrosArgs),

    /// Browse dbt seeds via the Discovery API
    #[command(
        long_about = "Browse dbt seeds via the Discovery API.\n\n\
            Inspect seed definitions. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp seeds show country_codes"
    )]
    Seeds(seeds::SeedsArgs),

    /// Browse dbt snapshots via the Discovery API
    #[command(
        long_about = "Browse dbt snapshots via the Discovery API.\n\n\
            Inspect snapshot definitions. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp snapshots show orders_snapshot"
    )]
    Snapshots(snapshots::SnapshotsArgs),

    /// Browse dbt tests via the Discovery API
    #[command(
        long_about = "Browse dbt tests via the Discovery API.\n\n\
            Inspect test definitions. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp tests show not_null_orders_id"
    )]
    Tests(tests::TestsArgs),

    /// Browse dbt semantic models via the Discovery API
    #[command(
        long_about = "Browse dbt semantic models via the Discovery API.\n\n\
            Inspect semantic model definitions. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp semantic-models show orders_semantic"
    )]
    SemanticModels(semantic_models::SemanticModelsArgs),

    // ── Semantic Layer API ─────────────────────────────
    /// Query and browse metrics via the Semantic Layer
    #[command(
        long_about = "Query and browse metrics via the Semantic Layer.\n\n\
            List metrics, explore their dimensions/entities/measures, execute metric\n\
            queries, or compile queries to SQL. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp metrics list\n  \
            dbtp metrics dimensions revenue\n  \
            dbtp metrics entities revenue\n  \
            dbtp metrics measures revenue\n  \
            dbtp metrics granularities revenue\n  \
            dbtp metrics for-dimensions customer_segment\n  \
            dbtp metrics query revenue --group-by metric_time --grain MONTH\n  \
            dbtp metrics query revenue --group-by metric_time,region \\\n    \
                --where \"metric_time >= '2025-01-01'\" --order-by -revenue --limit 10\n  \
            dbtp metrics sql revenue --group-by metric_time --grain DAY"
    )]
    Metrics(metrics::MetricsArgs),

    /// Browse saved queries via the Semantic Layer
    #[command(
        long_about = "Browse saved queries via the Semantic Layer.\n\n\
            List saved metric queries. Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp saved-queries list\n  \
            dbtp saved-queries list --search revenue"
    )]
    SavedQueries(saved_queries::SavedQueriesArgs),

    /// Query dimension values via the Semantic Layer
    #[command(
        long_about = "Query dimension values via the Semantic Layer.\n\n\
            Retrieve the distinct values of dimensions for given metrics.\n\
            Requires --environment-id.",
        after_long_help = "EXAMPLES:\n  \
            dbtp dimension-values list --metrics revenue --group-by customer_segment\n  \
            dbtp dimension-values list --metrics revenue,orders --group-by region,status"
    )]
    DimensionValues(dimension_values::DimensionValuesArgs),

    /// Manage the dbtp installation
    #[command(
        long_about = "Manage the dbtp installation.\n\n\
            Check version info, update to the latest release, or uninstall cleanly.",
        after_long_help = "EXAMPLES:\n  \
            dbtp system info\n  \
            dbtp system update\n  \
            dbtp system update --version v0.2.0\n  \
            dbtp system uninstall\n  \
            dbtp system uninstall --purge"
    )]
    System(system::SystemArgs),

    /// Generate shell completions
    #[command(
        long_about = "Generate shell completions for dbtp.\n\n\
            Output shell completion scripts for bash, zsh, fish, or PowerShell.\n\
            Add the output to your shell's config to enable tab completion.",
        after_long_help = "EXAMPLES:\n  \
            dbtp completion bash > ~/.bash_completion.d/dbtp\n  \
            dbtp completion zsh > ~/.zfunc/_dbtp\n  \
            dbtp completion fish > ~/.config/fish/completions/dbtp.fish"
    )]
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}
