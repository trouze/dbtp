use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::{json, Value};

use crate::api::admin::{environments, projects};
use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;
use crate::core::rest_client::RestClient;

use super::{extract_nodes, extract_path, metadata_url, paginate};

const GET_COLUMN_LINEAGE: &str = include_str!("queries/get_column_lineage.graphql");
const GET_MODELS_WITH_PATHS: &str = include_str!("queries/get_models_with_paths.graphql");
const GET_MODELS_WITH_ACCESS: &str = include_str!("queries/get_models_with_access.graphql");
const GET_FULL_LINEAGE: &str = include_str!("queries/get_full_lineage.graphql");
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

/// Fetch the set of unique_ids for all public models in `environment_id`.
async fn get_public_model_ids(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
) -> Result<HashSet<String>> {
    let models = paginate(
        client,
        host,
        environment_id,
        GET_MODELS_WITH_ACCESS,
        json!({}),
        &["environment", "applied", "models", "edges"],
        &["environment", "applied", "models", "pageInfo"],
    )
    .await?;

    Ok(models
        .into_iter()
        .filter(|m| m["access"].as_str() == Some("public"))
        .filter_map(|m| m["uniqueId"].as_str().map(String::from))
        .collect())
}

/// Walk downstream from `root_ids` in the node-level lineage graph and return the unique_ids of
/// any reachable model that is also in `public_model_ids`.
///
/// Uses `parentIds` from the full lineage graph rather than column-level edges, so it
/// correctly crosses `select *` references that column lineage cannot represent.
async fn find_downstream_public_via_node_lineage(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    root_ids: &[String],
    public_model_ids: &HashSet<String>,
) -> Result<Vec<String>> {
    let meta_host = metadata_url(host);

    let vars = json!({
        "environmentId": environment_id,
        "types": ["Model"],
    });

    let data = client
        .discovery(&meta_host, environment_id, GET_FULL_LINEAGE, Some(vars))
        .await?;

    let all_nodes = data["environment"]["applied"]["lineage"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    // Build children_of: parent_id -> [child_ids]
    let mut children_of: HashMap<String, Vec<String>> = HashMap::new();
    for node in &all_nodes {
        if let Some(uid) = node["uniqueId"].as_str() {
            if let Some(parents) = node["parentIds"].as_array() {
                for parent in parents {
                    if let Some(pid) = parent.as_str() {
                        children_of
                            .entry(pid.to_string())
                            .or_default()
                            .push(uid.to_string());
                    }
                }
            }
        }
    }

    // BFS downstream from root_ids
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    for id in root_ids {
        if visited.insert(id.clone()) {
            queue.push_back(id.clone());
        }
    }

    let mut downstream_public: Vec<String> = Vec::new();

    while let Some(current) = queue.pop_front() {
        if let Some(children) = children_of.get(&current) {
            for child in children {
                if visited.insert(child.clone()) {
                    if public_model_ids.contains(child) {
                        downstream_public.push(child.clone());
                    }
                    queue.push_back(child.clone());
                }
            }
        }
    }

    Ok(downstream_public)
}

/// Discover all deployment environment IDs in the account, excluding `exclude_env`.
async fn discover_deployment_envs(rest: &RestClient, exclude_env: u64) -> Result<Vec<u64>> {
    let all_projects = projects::list(rest, &[], None).await?;
    let mut env_ids: Vec<u64> = Vec::new();

    for proj in &all_projects {
        let Some(pid) = proj["id"].as_u64() else { continue };
        let envs = environments::list(rest, pid, &[], None).await?;
        for env in &envs {
            if env["type"].as_str() == Some("deployment") {
                if let Some(eid) = env["id"].as_u64() {
                    if eid != exclude_env {
                        env_ids.push(eid);
                    }
                }
            }
        }
    }

    Ok(env_ids)
}

/// Find models in `env_id` that declare any of `public_parent_ids` as a public parent.
///
/// Queries one public parent ID at a time so we can associate each consumer with the
/// specific public model it references.
async fn find_consumers_in_env(
    client: &GraphqlClient,
    host: &str,
    env_id: u64,
    public_parent_id: &str,
) -> Result<Vec<Value>> {
    let meta_host = metadata_url(host);
    let vars = json!({
        "environmentId": env_id,
        "filter": {
            "publicParents": [public_parent_id],
            "types": ["Model"],
        },
        "first": 100,
    });

    let data = client
        .discovery(&meta_host, env_id, GET_PUBLIC_PARENT_CONSUMERS, Some(vars))
        .await?;

    let edges = extract_path(&data, &["environment", "applied", "resources", "edges"]);
    Ok(extract_nodes(edges))
}

/// Analyze the impact of one or more changed models.
///
/// Phase 1 — same environment:
///   1. Walk node-level lineage to find downstream public models.
///   2. Fetch column-level lineage (downstream only), keep only columns landing in those
///      public models. These are the columns at risk of breaking downstream consumers.
///
/// Phase 2 — cross-environment:
///   For each impacted public model, query `downstream_envs` for mesh consumers.
///   When `downstream_envs` is empty, auto-discovers all deployment environments
///   in the account (slower but requires no configuration).
///   Consumer models are attached to each impact under the `consumers` key.
pub async fn build_impact_report(
    client: &GraphqlClient,
    rest: &RestClient,
    host: &str,
    environment_id: u64,
    unique_ids: &[String],
    downstream_envs: &[u64],
) -> Result<Value> {
    // ── Phase 1: columns that flow from changed models into downstream public models ──

    let public_model_ids = get_public_model_ids(client, host, environment_id).await?;

    let mut all_impacts: Vec<Value> = Vec::new();
    let mut no_public_downstream: Vec<String> = Vec::new();

    for uid in unique_ids {
        let mut downstream_public = find_downstream_public_via_node_lineage(
            client,
            host,
            environment_id,
            &[uid.clone()],
            &public_model_ids,
        )
        .await?;

        // If the input model itself is a public model, treat it as its own public surface.
        // The column filter (nodeUniqueId ∈ public_set) will then keep depth-0 columns
        // (the model's own columns), which are exactly the ones consumers depend on.
        if public_model_ids.contains(uid.as_str()) {
            downstream_public.push(uid.clone());
        }

        if downstream_public.is_empty() {
            no_public_downstream.push(uid.clone());
            continue;
        }

        let public_set: HashSet<&str> =
            downstream_public.iter().map(String::as_str).collect();

        let columns = fetch(client, host, environment_id, uid, true, false).await?;
        let column_list = columns.as_array().cloned().unwrap_or_default();

        for col in &column_list {
            let node_uid = col["nodeUniqueId"].as_str().unwrap_or("");
            if !public_set.contains(node_uid) {
                continue;
            }

            all_impacts.push(json!({
                "source_model": uid,
                "public_model": node_uid,
                "column_name": col["name"],
                "column_id": col["uniqueId"],
                "transformation": col["transformationType"],
                "depth": col["depth"],
                "consumers": [],
            }));
        }
    }

    // ── Phase 2: cross-environment consumer lookup ──

    // Unique public models that have at least one column impact.
    let impacted_public_models: Vec<String> = all_impacts
        .iter()
        .filter_map(|i| i["public_model"].as_str().map(String::from))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    if !impacted_public_models.is_empty() {
        let target_env_ids: Vec<u64> = if downstream_envs.is_empty() {
            eprintln!("discovering deployment environments in account...");
            discover_deployment_envs(rest, environment_id).await?
        } else {
            downstream_envs.to_vec()
        };

        // consumers_by_public_model: public_model_id -> [{model, project_id, environment_id}]
        let mut consumers_by_public_model: HashMap<String, Vec<Value>> = HashMap::new();

        for ds_env_id in &target_env_ids {
            for pm_id in &impacted_public_models {
                let consumers =
                    match find_consumers_in_env(client, host, *ds_env_id, pm_id).await {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!(
                                "warning: could not query env {ds_env_id} for consumers of {pm_id}: {e}"
                            );
                            continue;
                        }
                    };

                for node in consumers {
                    consumers_by_public_model
                        .entry(pm_id.clone())
                        .or_default()
                        .push(json!({
                            "model": node["uniqueId"],
                            "project_id": node["projectId"],
                            "environment_id": ds_env_id,
                        }));
                }
            }
        }

        // Attach consumers to each impact row.
        for impact in &mut all_impacts {
            let pm = impact["public_model"].as_str().unwrap_or("").to_string();
            if let Some(consumers) = consumers_by_public_model.get(&pm) {
                impact["consumers"] = json!(consumers);
            }
        }
    }

    // ── Summary ──

    let mut public_models_with_impact: Vec<String> = impacted_public_models.clone();
    public_models_with_impact.sort();

    let cross_project_consumers: Vec<&str> = all_impacts
        .iter()
        .flat_map(|i| {
            i["consumers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["model"].as_str())
                .collect::<Vec<_>>()
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    Ok(json!({
        "impacts": all_impacts,
        "summary": {
            "total_impacts": all_impacts.len(),
            "public_models_with_impact": public_models_with_impact,
            "cross_project_consumers": cross_project_consumers,
            "models_without_public_downstream": no_public_downstream,
        },
    }))
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
