# Contributing to `dbtp`

`dbtp` is a Rust CLI for the dbt Cloud Platform APIs.

1. [About this document](#about-this-document)
2. [Getting the code](#getting-the-code)
3. [Setting up an environment](#setting-up-an-environment)
4. [Running in development](#running-in-development)
5. [Testing](#testing)
6. [Adding a changelog entry](#adding-a-changelog-entry)
7. [Submitting a Pull Request](#submitting-a-pull-request)

## About this document

There are many ways to contribute — opening issues, joining discussions, or submitting code changes. This document focuses on the latter. It assumes familiarity with Rust and the command line.

If you get stuck, open a [GitHub Discussion](https://github.com/trouze/dbtp/discussions) or file an issue.

### Notes

- **Branches:** All pull requests should target the `main` branch.
- **CLA:** Contributions are accepted under the [Apache-2.0](LICENSE) license.

## Getting the code

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- `git`

### Fork and clone

External contributors should fork the repository, then clone their fork:

```sh
git clone https://github.com/<your-username>/dbtp.git
cd dbtp
```

dbt Labs contributors can clone directly and push to a feature branch.

## Setting up an environment

No additional setup is required beyond a working Rust toolchain. Install or update via:

```sh
rustup update stable
```

## Running in development

Build and run directly with `cargo`:

```sh
cargo build                        # debug build
cargo run -- projects list         # run a command
cargo run -- --help                # top-level help
```

Set environment variables to avoid passing flags every run:

```sh
export DBTP_TOKEN=<your-token>
export DBTP_ACCOUNT_ID=<your-account-id>
```

## Testing

Currently there is no automated test suite — commands are tested manually against a live dbt Cloud instance with the environment variables above.

When adding tests in the future, prefer unit tests for pure logic (URL construction, argument parsing, response formatting) that do not require network access.

```sh
cargo test    # runs any tests in the repo
```

## Code quality

CI enforces formatting and lints. Run these locally before pushing:

```sh
cargo fmt --check      # check formatting (run `cargo fmt` to fix)
cargo clippy -- -D warnings   # lints
```

## Adding a changelog entry

We use [changie](https://changie.dev) to manage the `CHANGELOG.md`. Do not edit `CHANGELOG.md` directly.

Install changie, then run:

```sh
changie new
```

Commit the generated file in `.changes/unreleased/` alongside your PR.

## Submitting a Pull Request

1. Create a branch off `main`.
2. Make your changes and ensure `cargo build`, `cargo fmt --check`, and `cargo clippy -- -D warnings` all pass.
3. Add a changelog entry with `changie new`.
4. Open a PR against `main`. A maintainer will review and merge.

CI runs automatically on all PRs. Once checks pass and the PR is approved, a maintainer will merge it.
