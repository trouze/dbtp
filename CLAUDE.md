# CLAUDE.md — Project Context for dbtp

## What is this?

`dbtp` is a Rust CLI for the dbt Cloud Platform APIs. It covers three API surfaces:

1. **Admin API** (REST v2/v3) — accounts, projects, environments, jobs, runs, artifacts
2. **Discovery API** (GraphQL) — models, sources, exposures, macros, seeds, snapshots, tests, semantic models, lineage
3. **Semantic Layer API** (GraphQL) — metrics, saved queries, dimension values, metric queries

The CLI follows hyperscaler conventions (`aws`, `az`, `gcloud`): flat `resource verb` hierarchy, consistent CRUD verbs, global output formatting, config profiles, auto-pagination, and envelope unwrapping.

## Repository layout

```
dbtp/
├── Cargo.toml                          # Rust project manifest (edition 2021, Apache-2.0)
├── src/
│   ├── main.rs                         # Entry point: parse CLI, load config, build clients, dispatch
│   ├── api/                            # API layer — request/response logic per API surface
│   │   ├── mod.rs
│   │   ├── types.rs                    # Shared API types (envelope, pagination)
│   │   ├── admin/                      # REST Admin API (v2 + v3)
│   │   │   ├── mod.rs                  # Compact formatters, re-exports
│   │   │   ├── accounts.rs
│   │   │   ├── projects.rs
│   │   │   ├── environments.rs
│   │   │   ├── jobs.rs
│   │   │   ├── runs.rs
│   │   │   └── artifacts.rs
│   │   ├── discovery/                  # GraphQL Discovery API
│   │   │   ├── mod.rs                  # GraphQL execution helpers, require_environment_id
│   │   │   ├── models.rs              # list, parents, children, health, performance
│   │   │   ├── lineage.rs
│   │   │   ├── sources.rs
│   │   │   ├── exposures.rs
│   │   │   ├── macros.rs
│   │   │   └── resource_details.rs    # Generic detail fetcher for any Discovery node type
│   │   └── semantic_layer/            # GraphQL Semantic Layer API
│   │       ├── mod.rs                 # SL URL construction, re-exports
│   │       ├── metrics.rs             # list, dimensions, entities, measures, granularities
│   │       ├── queries.rs             # execute_query, compile_sql (Arrow IPC decoding)
│   │       └── types.rs              # GroupByInput, OrderByInput, WhereInput, MetricInput
│   ├── cli/                           # CLI layer — clap definitions and command dispatch
│   │   ├── mod.rs                     # Cli struct, GlobalOpts, Commands enum (clap derive)
│   │   ├── output.rs                  # Output formatting: table (tabled), json, yaml, compact
│   │   └── commands/                  # One file per top-level command
│   │       ├── mod.rs                 # exec() dispatcher matching Commands enum
│   │       ├── configure.rs           # Interactive profile setup
│   │       ├── accounts.rs
│   │       ├── projects.rs
│   │       ├── environments.rs
│   │       ├── jobs.rs                # Includes trigger, trigger-from-failure
│   │       ├── runs.rs                # Includes cancel, retry, wait (polling), errors
│   │       ├── artifacts.rs
│   │       ├── models.rs              # Includes parents, children, health, performance
│   │       ├── lineage.rs
│   │       ├── sources.rs
│   │       ├── exposures.rs
│   │       ├── macros.rs
│   │       ├── seeds.rs
│   │       ├── snapshots.rs
│   │       ├── tests.rs
│   │       ├── semantic_models.rs
│   │       ├── metrics.rs             # Includes query, sql, dimensions, entities, etc.
│   │       ├── saved_queries.rs
│   │       └── dimension_values.rs
│   └── core/                          # Shared infrastructure
│       ├── mod.rs
│       ├── config.rs                  # Profile loading/saving, env var fallback, config.toml
│       ├── error.rs                   # DbtpError enum (Http, Config, GraphQL, Api, Io, Json, Arrow)
│       ├── rest_client.rs             # REST client: v2/v3 URL routing, auth, pagination, envelope unwrapping
│       └── graphql_client.rs          # GraphQL client for Discovery + Semantic Layer
├── openapi/                           # Reference OpenAPI specs (not used at build time)
│   ├── openapi-v2.yaml
│   └── openapi-v3.yaml
├── dbt-mcp/                           # Separate project: Python MCP server for dbt
├── dbtc/                              # Separate project: Python dbt Cloud API wrapper
└── .github/
    └── CODEOWNERS
```

## Key architecture decisions

- **Three-layer separation**: `core/` (config, clients, errors) → `api/` (request/response per API) → `cli/` (clap args, dispatch, output formatting). Commands in `cli/commands/` call into `api/` and return `serde_json::Value`; the dispatcher in `cli/commands/mod.rs` formats the result.
- **Everything is `serde_json::Value`**: Rather than strongly-typed response structs, commands return `Value` and the output layer formats generically. This keeps the code lean and makes adding new fields/endpoints trivial.
- **REST v2 vs v3**: The `RestClient` exposes `get_v2`/`get_v3`/etc. methods. v2 uses `Token` auth, v3 uses `Bearer` auth. Each `api/admin/*.rs` module knows which version its endpoints use. Users never see the version split.
- **GraphQL clients**: `GraphqlClient` handles both Discovery API (at `{host}/graphql`) and Semantic Layer (at a different URL constructed by `semantic_layer::semantic_layer_url`). Discovery uses environment_id as a query variable; Semantic Layer encodes it in the URL path.
- **Auto-pagination**: `RestClient::paginate_v2` / `paginate_v3` loop through offset-based pages (100 per request) until `total_count` is reached or `--limit` is satisfied.
- **Envelope unwrapping**: dbt Cloud REST APIs return `{ data, status, extra }`. The client strips this automatically via `unwrap_envelope()`.
- **Compact mode**: A `compact` output format provides single-line JSON and uses `compact_*` helpers in `api/admin/mod.rs` to reduce verbose API responses to essential fields.

## Building and running

```bash
cargo build                    # Debug build
cargo build --release          # Optimized build (strip + LTO)
cargo run -- projects list     # Run directly
```

The release profile enables `strip = true`, `lto = true`, `codegen-units = 1` for a small, fast binary.

## Configuration

Config lives at `~/.config/dbtp/config.toml` (macOS) via the `directories` crate's `ProjectDirs::from("com", "dbt-labs", "dbtp")`.

Precedence: CLI flags > env vars (`DBTP_TOKEN`, `DBTP_HOST`, `DBTP_ACCOUNT_ID`, `DBTP_ENVIRONMENT_ID`) > profile config > defaults.

## Key dependencies

| Crate | Purpose |
|---|---|
| `clap` (derive) | CLI framework with subcommands and env var integration |
| `reqwest` (rustls-tls) | Async HTTP client |
| `tokio` | Async runtime |
| `serde` / `serde_json` / `serde_yaml` | Serialization |
| `tabled` | Table output rendering |
| `toml` | Config file parsing |
| `thiserror` | Error type derivation |
| `directories` | XDG-compliant config path resolution |
| `indicatif` | Progress bars |
| `chrono` | Timestamp handling |
| `arrow` (ipc) | Decoding Semantic Layer query results (Arrow IPC format) |
| `base64` | Decoding base64-encoded Arrow payloads |

## Sibling projects in this repo

- **`dbt-mcp/`** — Python MCP (Model Context Protocol) server for dbt. Separate project with its own dependencies and entry point. Not part of the Rust binary.
- **`dbtc/`** — Python dbt Cloud API client library and CLI. Separate project. Not part of the Rust binary.
- **`openapi/`** — Reference OpenAPI v2/v3 specs from dbt Cloud. Used as a reference when writing API modules, not consumed at build time.

## Adding a new command

1. Add the API module in `src/api/{surface}/` (e.g. `src/api/admin/connections.rs`).
2. Add the command module in `src/cli/commands/` (e.g. `connections.rs`) with `*Args`, `*Command` enum, and `exec()`.
3. Register the command in `src/cli/mod.rs` (add variant to `Commands` enum) and `src/cli/commands/mod.rs` (add to `exec()` match and module imports).
4. The output layer handles formatting automatically — just return `serde_json::Value` from your `exec()`.

## Releasing

Push a `v*` tag to trigger the release workflow at `.github/workflows/release.yml`. It cross-compiles for macOS (arm64 + x86_64) and Linux (x86_64 + arm64), then creates a GitHub Release with tarballs and SHA-256 checksums. The `install.sh` script at the repo root fetches the latest release for the user's platform.

```bash
git tag v0.1.0 && git push --tags
```

## Testing

No test suite currently. The CLI is tested manually against live dbt Cloud instances. Set `DBTP_TOKEN` and `DBTP_ACCOUNT_ID` in your environment and run commands directly.
