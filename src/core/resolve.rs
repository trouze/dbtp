use crate::api::admin::{environments, projects};
use crate::core::error::{DbtpError, Result};
use crate::core::rest_client::RestClient;

/// Resolve a project identifier (numeric ID or name) to a numeric ID.
/// If the input parses as u64, it is returned directly with zero API calls.
/// Otherwise, lists all projects and performs a case-insensitive exact match on name.
pub async fn resolve_project(client: &RestClient, input: &str) -> Result<u64> {
    if let Ok(id) = input.parse::<u64>() {
        return Ok(id);
    }

    let projects = projects::list(client, &[], None).await?;
    let mut matches: Vec<(u64, String)> = Vec::new();
    let needle = input.to_lowercase();

    for p in &projects {
        let name = p["name"].as_str().unwrap_or_default();
        if name.to_lowercase() == needle {
            if let Some(id) = p["id"].as_u64() {
                matches.push((id, name.to_string()));
            }
        }
    }

    match matches.len() {
        0 => Err(DbtpError::config(format!(
            "No project found matching \"{input}\""
        ))),
        1 => Ok(matches[0].0),
        _ => {
            let candidates: Vec<String> = matches
                .iter()
                .map(|(id, name)| format!("{name} ({id})"))
                .collect();
            Err(DbtpError::config(format!(
                "Multiple projects match \"{input}\": {}. Use --project-id <numeric_id> to disambiguate.",
                candidates.join(", ")
            )))
        }
    }
}

/// Resolve an environment identifier (numeric ID or name) to a numeric ID.
/// Requires a project_id to scope the name search. If no project_id is available
/// and the input is non-numeric, returns an error.
pub async fn resolve_environment(
    client: &RestClient,
    project_id: Option<u64>,
    input: &str,
) -> Result<u64> {
    if let Ok(id) = input.parse::<u64>() {
        return Ok(id);
    }

    let pid = project_id.ok_or_else(|| {
        DbtpError::config(
            "project_id is required to resolve an environment by name; \
             set via --project-id, DBTP_PROJECT_ID, or `dbtp configure`",
        )
    })?;

    let envs = environments::list(client, pid, &[], None).await?;
    let mut matches: Vec<(u64, String)> = Vec::new();
    let needle = input.to_lowercase();

    for e in &envs {
        let name = e["name"].as_str().unwrap_or_default();
        if name.to_lowercase() == needle {
            if let Some(id) = e["id"].as_u64() {
                matches.push((id, name.to_string()));
            }
        }
    }

    match matches.len() {
        0 => Err(DbtpError::config(format!(
            "No environment found matching \"{input}\" in project {pid}"
        ))),
        1 => Ok(matches[0].0),
        _ => {
            let candidates: Vec<String> = matches
                .iter()
                .map(|(id, name)| format!("{name} ({id})"))
                .collect();
            Err(DbtpError::config(format!(
                "Multiple environments match \"{input}\": {}. Use --environment-id <numeric_id> to disambiguate.",
                candidates.join(", ")
            )))
        }
    }
}
