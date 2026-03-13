# dbtp — dbt Cloud Platform CLI

A fast, ergonomic command-line interface for the [dbt Cloud](https://www.getdbt.com/product/dbt-cloud) platform APIs. Manage accounts, projects, environments, jobs, and runs. Browse your dbt DAG through the Discovery API. Query metrics through the Semantic Layer. All from your terminal.

```
dbtp jobs trigger 48213 --cause "deploy from CI"
dbtp runs wait 901244 --interval 15
dbtp models health orders
dbtp metrics query revenue --group-by metric_time --grain MONTH
```

## Installation

### One-liner (macOS / Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/trouze/dbtp/main/install.sh | bash
```

This downloads the latest prebuilt binary from [GitHub Releases](https://github.com/trouze/dbtp/releases) and installs it to `/usr/local/bin`. Set `DBTP_INSTALL_DIR` to customize the location.

### From source via Git (requires Rust)

```bash
cargo install --git https://github.com/trouze/dbtp
```

### From a local clone

```bash
git clone https://github.com/trouze/dbtp.git
cd dbtp
cargo install --path .
```

## Quick Start

### 1. Configure credentials

```bash
dbtp configure
```

You'll be prompted for your dbt Cloud host, API token, account ID, and (optionally) an environment ID for Discovery/Semantic Layer commands. Configuration is saved to `~/.config/dbtp/config.toml`.

### 2. Verify access

```bash
dbtp accounts list
```

### 3. Start using it

```bash
dbtp projects list
dbtp jobs list --project-id 12345
dbtp runs show 901244 -o json
dbtp models list --environment-id 67890
```

## Configuration

### Config file

`~/.config/dbtp/config.toml` supports named profiles:

```toml
[defaults]
profile = "prod"
output = "table"

[profile.prod]
host = "https://cloud.getdbt.com"
token = "dbtc_..."
account_id = 51798

[profile.staging]
host = "https://emea.dbt.com"
token = "dbtc_..."
account_id = 99999
environment_id = 67890
```

### Environment variables

| Variable | Description |
|---|---|
| `DBTP_HOST` | dbt Cloud host URL |
| `DBTP_TOKEN` | API token (service token or personal access token) |
| `DBTP_ACCOUNT_ID` | Default account ID |
| `DBTP_ENVIRONMENT_ID` | Default environment ID (Discovery / Semantic Layer) |

### Precedence

CLI flags > environment variables > profile config > defaults.

### Switching profiles

```bash
dbtp projects list --profile staging
```

## Commands

`dbtp` follows a **`resource verb`** pattern inspired by hyperscaler CLIs (`aws`, `az`, `gcloud`).

### Admin API

Manage your dbt Cloud account resources.

| Command | Verbs |
|---|---|
| `accounts` | `list`, `show` |
| `projects` | `list`, `show`, `create`, `update`, `delete` |
| `environments` | `list`, `show`, `create`, `update`, `delete` |
| `jobs` | `list`, `show`, `create`, `update`, `delete`, `trigger`, `trigger-from-failure` |
| `runs` | `list`, `show`, `cancel`, `retry`, `wait`, `errors` |
| `artifacts` | `list`, `get` |

### Discovery API

Browse your dbt project's DAG, metadata, and lineage. Requires `--environment-id` (or set it in your config/env).

| Command | Verbs |
|---|---|
| `models` | `list`, `show`, `parents`, `children`, `health`, `performance` |
| `sources` | `list`, `show` |
| `exposures` | `list`, `show` |
| `macros` | `list`, `show` |
| `seeds` | `list`, `show` |
| `snapshots` | `list`, `show` |
| `tests` | `list`, `show` |
| `semantic-models` | `list`, `show` |
| `lineage` | `show` |

### Semantic Layer API

Query metrics defined in your dbt project. Requires `--environment-id`.

| Command | Verbs |
|---|---|
| `metrics` | `list`, `dimensions`, `entities`, `measures`, `granularities`, `for-dimensions`, `query`, `sql` |
| `saved-queries` | `list` |
| `dimension-values` | `list` |

## Output Formats

Control output with `--output` / `-o`:

| Format | Description |
|---|---|
| `table` | Human-readable table (default) |
| `json` | Pretty-printed JSON |
| `yaml` | YAML |
| `compact` | Single-line JSON (piping-friendly) |

```bash
# Default table output
dbtp projects list

# JSON for scripting
dbtp runs show 901244 -o json

# Pipe-friendly compact JSON
dbtp jobs list -o compact | jq '.[].name'
```

## Usage Examples

### Trigger a job and wait for completion

```bash
dbtp jobs trigger 48213 --cause "nightly refresh"
dbtp runs wait 901244 --interval 10 --timeout 1800
```

### Inspect a failed run

```bash
dbtp runs errors 901244
```

### Browse model lineage

```bash
dbtp models parents orders
dbtp models children stg_payments
dbtp lineage show model.analytics.orders
```

### Check model health

```bash
dbtp models health orders
dbtp models performance orders --num-runs 20 --include-tests
```

### Query metrics

```bash
# List available metrics
dbtp metrics list

# Explore metric dimensions
dbtp metrics dimensions revenue

# Execute a query
dbtp metrics query revenue \
  --group-by metric_time,customer_segment \
  --grain MONTH \
  --where "metric_time >= '2025-01-01'" \
  --order-by -revenue \
  --limit 10

# Preview the compiled SQL
dbtp metrics sql revenue --group-by metric_time --grain DAY
```

### Download artifacts

```bash
dbtp artifacts list 901244
dbtp artifacts get 901244 manifest.json
dbtp artifacts get 901244 run_results.json
```

## Global Flags

| Flag | Short | Description |
|---|---|---|
| `--output <format>` | `-o` | Output format: `table`, `json`, `yaml`, `compact` |
| `--profile <name>` | | Named config profile |
| `--host <url>` | | dbt Cloud host URL |
| `--token <token>` | | API token |
| `--account-id <id>` | | dbt Cloud account ID |
| `--environment-id <id>` | | Environment ID (Discovery / Semantic Layer) |
| `--verbose` | `-v` | Enable verbose output |
| `--query <jmespath>` | | JMESPath expression to filter JSON output |

## Shell Completions

Generate tab-completion scripts for your shell:

```bash
# Bash
dbtp completion bash > ~/.bash_completion.d/dbtp

# Zsh
dbtp completion zsh > ~/.zfunc/_dbtp

# Fish
dbtp completion fish > ~/.config/fish/completions/dbtp.fish

# PowerShell
dbtp completion powershell > _dbtp.ps1
```

## How It Works

- **API version routing** is handled automatically. The v2/v3 split in dbt Cloud's API is invisible to users — each command knows which version to use.
- **Response envelope unwrapping** — dbt Cloud APIs return responses wrapped in `{ data, status, extra }`. The CLI strips the envelope and presents `data` directly.
- **Auto-pagination** — `list` commands transparently paginate through all results. Use `--limit` to cap the total.
- **Waiters** — `dbtp runs wait` polls until a run reaches a terminal state, printing status transitions as they happen.

## Releasing

Releases are automated via GitHub Actions. To cut a release:

```bash
# Update version in Cargo.toml, then:
git commit -am "release: v0.1.0"
git tag v0.1.0
git push && git push --tags
```

The [release workflow](.github/workflows/release.yml) builds binaries for macOS (arm64 + x86_64) and Linux (x86_64 + arm64), then creates a GitHub Release with all artifacts and checksums.

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.
