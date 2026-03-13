use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::{json, Value};

use crate::core::error::Result;
use crate::core::graphql_client::GraphqlClient;

use super::metadata_url;

const GET_FULL_LINEAGE: &str = include_str!("queries/get_full_lineage.graphql");

const ALL_LINEAGE_TYPES: &[&str] = &[
    "Analysis",
    "Exposure",
    "Model",
    "Seed",
    "Snapshot",
    "Source",
    "Test",
];

/// Fetch lineage graph filtered to nodes connected to `unique_id`.
///
/// `depth`: 0 = infinite, N = N hops from the target.
/// `types`: resource types to include; if empty, uses all types.
pub async fn fetch(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    unique_id: &str,
    depth: u32,
    types: &[String],
) -> Result<Value> {
    let meta_host = metadata_url(host);

    let type_filter: Vec<&str> = if types.is_empty() {
        ALL_LINEAGE_TYPES.to_vec()
    } else {
        types.iter().map(|s| s.as_str()).collect()
    };

    let vars = json!({
        "environmentId": environment_id,
        "types": type_filter,
    });

    let data = client
        .discovery(&meta_host, environment_id, GET_FULL_LINEAGE, Some(vars))
        .await?;

    let all_nodes = data["environment"]["applied"]["lineage"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let filtered = filter_connected_nodes(&all_nodes, unique_id, depth);
    Ok(Value::Array(filtered))
}

/// BFS from `target_id` in both directions (upstream via parentIds, downstream via reverse lookup).
/// Excludes macros. depth=0 means infinite.
fn filter_connected_nodes(nodes: &[Value], target_id: &str, depth: u32) -> Vec<Value> {
    let node_map: HashMap<&str, &Value> = nodes
        .iter()
        .filter_map(|n| {
            let uid = n["uniqueId"].as_str()?;
            let rt = n["resourceType"].as_str()?;
            if rt.eq_ignore_ascii_case("macro") {
                return None;
            }
            Some((uid, n))
        })
        .collect();

    if !node_map.contains_key(target_id) {
        return vec![];
    }

    // Build reverse index: child -> parents it is a child of
    // (node_map has parent_ids on each node; build child_ids for reverse traversal)
    let mut children_of: HashMap<&str, Vec<&str>> = HashMap::new();
    for (&uid, node) in &node_map {
        if let Some(parents) = node["parentIds"].as_array() {
            for parent in parents {
                if let Some(pid) = parent.as_str() {
                    if node_map.contains_key(pid) {
                        children_of.entry(pid).or_default().push(uid);
                    }
                }
            }
        }
    }

    let mut connected: HashSet<&str> = HashSet::new();
    connected.insert(target_id);
    let mut queue: VecDeque<(&str, u32)> = VecDeque::new();
    queue.push_back((target_id, 0));

    while let Some((current_id, current_depth)) = queue.pop_front() {
        if depth > 0 && current_depth >= depth {
            continue;
        }

        let Some(node) = node_map.get(current_id) else {
            continue;
        };

        // Traverse upstream (parents)
        if let Some(parents) = node["parentIds"].as_array() {
            for parent in parents {
                if let Some(pid) = parent.as_str() {
                    if node_map.contains_key(pid) && connected.insert(pid) {
                        queue.push_back((pid, current_depth + 1));
                    }
                }
            }
        }

        // Traverse downstream (children)
        if let Some(child_list) = children_of.get(current_id) {
            for &child_id in child_list {
                if connected.insert(child_id) {
                    queue.push_back((child_id, current_depth + 1));
                }
            }
        }
    }

    nodes
        .iter()
        .filter(|n| {
            n["uniqueId"]
                .as_str()
                .map_or(false, |uid| connected.contains(uid))
        })
        .cloned()
        .collect()
}
