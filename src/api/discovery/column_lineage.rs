use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::{json, Value};

use crate::api::admin::{environments, projects};
use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;
use crate::core::rest_client::RestClient;

use super::{extract_nodes, extract_path, metadata_url, paginate};

const GET_COLUMN_LINEAGE: &str = include_str!("queries/get_column_lineage.graphql");
const GET_MODELS_WITH_PATHS: &str = include_str!("queries/get_models_with_paths.graphql");
const GET_PUBLIC_PARENT_CONSUMERS: &str = include_str!("queries/get_public_parent_consumers.graphql");

/// Fetch column-level lineage for a single model.
///
/// Returns all column nodes in the lineage graph reachable from `node_unique_id`.
/// Use `downstream_only` / `upstream_only` to filter by direction.
///
/// The beta API's `depth` field is unreliable (often 0 for all nodes), so we
/// compute relative depth via BFS from the target model's columns: upstream
/// columns get negative depth, downstream get positive.
pub async fn fetch(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    node_unique_id: &str,
    downstream_only: bool,
    upstream_only: bool,
) -> Result<Value> {
    let meta_host = metadata_url(host);

    let vars = json!({
        "environmentId": environment_id,
        "nodeUniqueId": node_unique_id,
    });

    let data = client
        .discovery_beta(&meta_host, environment_id, GET_COLUMN_LINEAGE, Some(vars))
        .await?;

    let mut all_columns = data["column"]["lineage"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    if downstream_only || upstream_only {
        let depths = compute_relative_depths(&all_columns, node_unique_id);

        all_columns = all_columns
            .into_iter()
            .filter(|col| {
                let uid = col["uniqueId"].as_str().unwrap_or("");
                let depth = depths.get(uid).copied().unwrap_or(0);
                if downstream_only {
                    depth >= 0
                } else {
                    depth <= 0
                }
            })
            .collect();
    }

    rewrite_depths(&mut all_columns, node_unique_id);

    Ok(Value::Array(all_columns))
}

/// BFS from columns belonging to `target_node_id` to assign relative depths.
/// Depth 0 = target model's own columns, negative = upstream, positive = downstream.
fn compute_relative_depths(columns: &[Value], target_node_id: &str) -> HashMap<String, i64> {
    let col_index: HashMap<&str, &Value> = columns
        .iter()
        .filter_map(|c| c["uniqueId"].as_str().map(|id| (id, c)))
        .collect();

    let mut depths: HashMap<String, i64> = HashMap::new();
    let mut queue: VecDeque<(String, i64)> = VecDeque::new();

    for col in columns {
        if col["nodeUniqueId"].as_str() == Some(target_node_id) {
            if let Some(uid) = col["uniqueId"].as_str() {
                depths.insert(uid.to_string(), 0);
                queue.push_back((uid.to_string(), 0));
            }
        }
    }

    while let Some((current_id, current_depth)) = queue.pop_front() {
        let Some(col) = col_index.get(current_id.as_str()) else {
            continue;
        };

        if let Some(children) = col["childColumns"].as_array() {
            for child in children {
                if let Some(child_id) = child.as_str() {
                    if col_index.contains_key(child_id) && !depths.contains_key(child_id) {
                        depths.insert(child_id.to_string(), current_depth + 1);
                        queue.push_back((child_id.to_string(), current_depth + 1));
                    }
                }
            }
        }

        if let Some(parents) = col["parentColumns"].as_array() {
            for parent in parents {
                if let Some(parent_id) = parent.as_str() {
                    if col_index.contains_key(parent_id) && !depths.contains_key(parent_id) {
                        depths.insert(parent_id.to_string(), current_depth - 1);
                        queue.push_back((parent_id.to_string(), current_depth - 1));
                    }
                }
            }
        }
    }

    depths
}

/// Overwrite the `depth` field on each column with our computed relative depth.
fn rewrite_depths(columns: &mut [Value], target_node_id: &str) {
    let depths = compute_relative_depths(columns, target_node_id);
    for col in columns.iter_mut() {
        if let Some(uid) = col["uniqueId"].as_str() {
            if let Some(&d) = depths.get(uid) {
                col["depth"] = json!(d);
            }
        }
    }
}

/// Resolve file paths to model unique_ids by querying the Discovery API for models with filePath.
///
/// Returns `(resolved_unique_ids, unmatched_paths)`. Matching is done via suffix: an input path
/// matches a model if the model's `filePath` ends with the input path (or vice versa).
pub async fn resolve_files_to_unique_ids(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    file_paths: &[String],
) -> Result<(Vec<String>, Vec<String>)> {
    let models = paginate(
        client,
        host,
        environment_id,
        GET_MODELS_WITH_PATHS,
        json!({}),
        &["environment", "applied", "models", "edges"],
        &["environment", "applied", "models", "pageInfo"],
    )
    .await?;

    let mut resolved = Vec::new();
    let mut unmatched = Vec::new();

    for input_path in file_paths {
        let normalized = normalize_path(input_path);
        let mut found = false;

        for model in &models {
            let model_path = model["filePath"].as_str().unwrap_or("");
            let normalized_model = normalize_path(model_path);

            if suffix_match(&normalized, &normalized_model) {
                if let Some(uid) = model["uniqueId"].as_str() {
                    resolved.push(uid.to_string());
                    found = true;
                    break;
                }
            }
        }

        if !found {
            unmatched.push(input_path.clone());
        }
    }

    Ok((resolved, unmatched))
}

/// Discover all deployment environment IDs in the account, excluding `exclude_env`.
async fn discover_deployment_envs(
    rest: &RestClient,
    exclude_env: u64,
) -> Result<Vec<u64>> {
    let all_projects = projects::list(rest, &[], None).await?;
    let mut env_ids: Vec<u64> = Vec::new();

    for proj in &all_projects {
        let Some(pid) = proj["id"].as_u64() else { continue };
        let envs = environments::list(rest, pid, &[], None).await?;
        for env in &envs {
            let is_deployment = env["type"].as_str() == Some("deployment");
            if let Some(eid) = env["id"].as_u64() {
                if is_deployment && eid != exclude_env {
                    env_ids.push(eid);
                }
            }
        }
    }

    Ok(env_ids)
}

/// Find models in `env_id` that consume any of the given `public_parent_ids` via mesh.
async fn find_consumers_in_env(
    client: &GraphqlClient,
    host: &str,
    env_id: u64,
    public_parent_ids: &[String],
) -> Result<Vec<String>> {
    let meta_host = metadata_url(host);
    let filter = json!({
        "publicParents": public_parent_ids,
        "types": ["Model"],
    });
    let vars = json!({
        "environmentId": env_id,
        "filter": filter,
        "first": 100,
    });

    let data = client
        .discovery(&meta_host, env_id, GET_PUBLIC_PARENT_CONSUMERS, Some(vars))
        .await?;

    let edges = extract_path(&data, &["environment", "applied", "resources", "edges"]);
    let nodes = extract_nodes(edges);

    Ok(nodes
        .iter()
        .filter_map(|n| n["uniqueId"].as_str().map(String::from))
        .collect())
}

/// Build a cross-project impact report for one or more modified models.
///
/// For each model, fetches its full column lineage, then identifies downstream columns
/// that belong to a different project. When `downstream_envs` is non-empty, also queries
/// those environments for cross-project consumers via mesh. When empty, auto-discovers
/// all deployment environments in the account.
pub async fn build_impact_report(
    client: &GraphqlClient,
    rest: &RestClient,
    host: &str,
    environment_id: u64,
    unique_ids: &[String],
    cross_project_only: bool,
    downstream_envs: &[u64],
) -> Result<Value> {
    let mut all_impacts: Vec<Value> = Vec::new();
    let mut models_analyzed: usize = 0;
    let mut impacted_projects: HashSet<String> = HashSet::new();
    let mut impacted_models: HashSet<String> = HashSet::new();

    // Determine the source project ID from the first model's column lineage
    let mut source_project_id = String::new();

    for uid in unique_ids {
        let columns = fetch(client, host, environment_id, uid, false, false).await?;
        let column_list = columns.as_array().cloned().unwrap_or_default();

        if column_list.is_empty() {
            continue;
        }
        models_analyzed += 1;

        if source_project_id.is_empty() {
            source_project_id = column_list
                .iter()
                .find(|c| c["nodeUniqueId"].as_str() == Some(uid.as_str()))
                .and_then(|c| c["projectId"].as_u64().or_else(|| c["projectId"].as_str().and_then(|s| s.parse().ok())))
                .map(|id| id.to_string())
                .unwrap_or_default();
        }

        let spid = column_list
            .iter()
            .find(|c| {
                c["nodeUniqueId"].as_str() == Some(uid.as_str())
                    && c["depth"].as_i64() == Some(0)
            })
            .and_then(|c| c["projectId"].as_str())
            .or_else(|| {
                column_list
                    .iter()
                    .find(|c| c["depth"].as_i64() == Some(0))
                    .and_then(|c| c["projectId"].as_str())
            })
            .unwrap_or("");

        let col_map: HashMap<&str, &Value> = column_list
            .iter()
            .filter_map(|c| c["uniqueId"].as_str().map(|id| (id, c)))
            .collect();

        let source_columns: Vec<&str> = column_list
            .iter()
            .filter(|c| c["nodeUniqueId"].as_str() == Some(uid.as_str()))
            .filter_map(|c| c["uniqueId"].as_str())
            .collect();

        let downstream = bfs_downstream(&col_map, &source_columns);

        for col_id in downstream {
            let Some(col) = col_map.get(col_id) else {
                continue;
            };

            let col_project = col["projectId"].as_str().unwrap_or("");
            let is_cross_project = !spid.is_empty()
                && !col_project.is_empty()
                && col_project != spid;

            if cross_project_only && !is_cross_project {
                continue;
            }

            if col["nodeUniqueId"].as_str() == Some(uid.as_str()) {
                continue;
            }

            let node_uid = col["nodeUniqueId"].as_str().unwrap_or("");
            if is_cross_project {
                impacted_projects.insert(col_project.to_string());
                impacted_models.insert(node_uid.to_string());
            }

            let parent_cols = col["parentColumns"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let relevant_sources: Vec<&str> = parent_cols
                .iter()
                .filter(|p| {
                    col_map
                        .get(*p)
                        .and_then(|c| c["depth"].as_i64())
                        .map_or(false, |d| d >= 0)
                })
                .copied()
                .collect();

            all_impacts.push(json!({
                "source_model": uid,
                "source_project": spid,
                "impacted_column": col["name"].as_str().unwrap_or(""),
                "impacted_column_id": col_id,
                "impacted_model": node_uid,
                "impacted_project": col_project,
                "transformation": col["transformationType"].as_str().unwrap_or(""),
                "is_cross_project": is_cross_project,
                "depth": col["depth"],
                "parent_columns": relevant_sources,
            }));
        }
    }

    // Cross-environment lookup: query downstream environments for mesh consumers
    let target_env_ids = if downstream_envs.is_empty() {
        eprintln!("Discovering deployment environments in account...");
        discover_deployment_envs(rest, environment_id).await?
    } else {
        downstream_envs.to_vec()
    };

    for ds_env_id in &target_env_ids {
        let consumers = match find_consumers_in_env(client, host, *ds_env_id, unique_ids).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: could not query environment {ds_env_id}: {e}");
                continue;
            }
        };

        if consumers.is_empty() {
            continue;
        }

        for consumer_uid in &consumers {
            let columns = match fetch(client, host, *ds_env_id, consumer_uid, false, false).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "warning: could not fetch column lineage for {consumer_uid} in env {ds_env_id}: {e}"
                    );
                    continue;
                }
            };
            let column_list = columns.as_array().cloned().unwrap_or_default();
            if column_list.is_empty() {
                continue;
            }

            let col_map: HashMap<&str, &Value> = column_list
                .iter()
                .filter_map(|c| c["uniqueId"].as_str().map(|id| (id, c)))
                .collect();

            // Start BFS from columns belonging to the upstream (source) models
            for uid in unique_ids {
                let upstream_cols: Vec<&str> = column_list
                    .iter()
                    .filter(|c| c["nodeUniqueId"].as_str() == Some(uid.as_str()))
                    .filter_map(|c| c["uniqueId"].as_str())
                    .collect();

                if upstream_cols.is_empty() {
                    continue;
                }
                models_analyzed += 1;

                let downstream = bfs_downstream(&col_map, &upstream_cols);

                for col_id in downstream {
                    let Some(col) = col_map.get(col_id) else { continue };

                    let col_node = col["nodeUniqueId"].as_str().unwrap_or("");
                    // Skip columns that belong to the source model itself
                    if col_node == uid.as_str() {
                        continue;
                    }

                    let col_project = col["projectId"]
                        .as_u64()
                        .map(|n| n.to_string())
                        .or_else(|| col["projectId"].as_str().map(String::from))
                        .unwrap_or_default();

                    let is_cross = !source_project_id.is_empty()
                        && !col_project.is_empty()
                        && col_project != source_project_id;

                    if cross_project_only && !is_cross {
                        continue;
                    }

                    if is_cross {
                        impacted_projects.insert(col_project.clone());
                        impacted_models.insert(col_node.to_string());
                    }

                    all_impacts.push(json!({
                        "source_model": uid,
                        "source_project": source_project_id,
                        "impacted_column": col["name"].as_str().unwrap_or(""),
                        "impacted_column_id": col_id,
                        "impacted_model": col_node,
                        "impacted_project": col_project,
                        "transformation": col["transformationType"].as_str().unwrap_or(""),
                        "is_cross_project": is_cross,
                        "depth": col["depth"],
                        "downstream_environment_id": ds_env_id,
                        "parent_columns": col["parentColumns"],
                    }));
                }
            }
        }
    }

    let mut sorted_projects: Vec<String> = impacted_projects.into_iter().collect();
    sorted_projects.sort();
    let mut sorted_models: Vec<String> = impacted_models.into_iter().collect();
    sorted_models.sort();

    Ok(json!({
        "impacts": all_impacts,
        "summary": {
            "models_analyzed": models_analyzed,
            "total_impacts": all_impacts.len(),
            "cross_project_impacts": all_impacts.iter()
                .filter(|i| i["is_cross_project"].as_bool() == Some(true))
                .count(),
            "impacted_projects": sorted_projects,
            "impacted_models": sorted_models,
        }
    }))
}

/// BFS downstream from a set of starting column IDs via childColumns edges.
/// Returns all reachable column IDs (excluding the starting set).
fn bfs_downstream<'a>(
    col_map: &HashMap<&'a str, &Value>,
    start_ids: &[&'a str],
) -> Vec<&'a str> {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    for &id in start_ids {
        visited.insert(id);
        queue.push_back(id);
    }

    let mut result: Vec<&str> = Vec::new();

    while let Some(current) = queue.pop_front() {
        let Some(col) = col_map.get(current) else {
            continue;
        };

        let children = col["childColumns"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for child_id in children {
            if let Some(&child_key) = col_map.keys().find(|&&k| k == child_id) {
                if visited.insert(child_key) {
                    // Skip children ending with "*" (wildcard nodes)
                    if !child_key.ends_with('*') {
                        result.push(child_key);
                        queue.push_back(child_key);
                    }
                }
            }
        }
    }

    result
}

fn normalize_path(path: &str) -> String {
    path.trim().replace('\\', "/").to_lowercase()
}

/// Suffix-based path matching: returns true if either path ends with the other.
fn suffix_match(input: &str, model_path: &str) -> bool {
    if input.is_empty() || model_path.is_empty() {
        return false;
    }
    model_path.ends_with(input) || input.ends_with(model_path)
}
