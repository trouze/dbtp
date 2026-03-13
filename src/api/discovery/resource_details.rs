use serde_json::{json, Value};

use crate::core::error::{DbtpError, Result};
use crate::core::graphql_client::GraphqlClient;

use super::{extract_nodes, metadata_url};

const GET_MODEL_DETAILS: &str = include_str!("queries/get_model_details.graphql");
const GET_SOURCE_DETAILS: &str = include_str!("queries/get_source_details.graphql");
const GET_EXPOSURE_DETAILS: &str = include_str!("queries/get_exposure_details.graphql");
const GET_TEST_DETAILS: &str = include_str!("queries/get_test_details.graphql");
const GET_SEED_DETAILS: &str = include_str!("queries/get_seed_details.graphql");
const GET_SNAPSHOT_DETAILS: &str = include_str!("queries/get_snapshot_details.graphql");
const GET_MACRO_DETAILS: &str = include_str!("queries/get_macro_details.graphql");
const GET_SEMANTIC_MODEL_DETAILS: &str = include_str!("queries/get_semantic_model_details.graphql");
const GET_PACKAGES: &str = include_str!("queries/get_packages.graphql");

#[derive(Debug, Clone, Copy)]
pub enum ResourceType {
    Model,
    Source,
    Exposure,
    Test,
    Seed,
    Snapshot,
    Macro,
    SemanticModel,
}

impl ResourceType {
    fn query(&self) -> &'static str {
        match self {
            Self::Model => GET_MODEL_DETAILS,
            Self::Source => GET_SOURCE_DETAILS,
            Self::Exposure => GET_EXPOSURE_DETAILS,
            Self::Test => GET_TEST_DETAILS,
            Self::Seed => GET_SEED_DETAILS,
            Self::Snapshot => GET_SNAPSHOT_DETAILS,
            Self::Macro => GET_MACRO_DETAILS,
            Self::SemanticModel => GET_SEMANTIC_MODEL_DETAILS,
        }
    }

    fn graphql_type_name(&self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Source => "Source",
            Self::Exposure => "Exposure",
            Self::Test => "Test",
            Self::Seed => "Seed",
            Self::Snapshot => "Snapshot",
            Self::Macro => "Macro",
            Self::SemanticModel => "SemanticModel",
        }
    }

    fn id_prefix(&self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Source => "source",
            Self::Exposure => "exposure",
            Self::Test => "test",
            Self::Seed => "seed",
            Self::Snapshot => "snapshot",
            Self::Macro => "macro",
            Self::SemanticModel => "semantic_model",
        }
    }
}

/// Fetch detailed resource information by name or unique_id.
///
/// If `identifier` contains dots, it's treated as a unique_id.
/// Otherwise, it's treated as a name and resolved via GetPackages.
pub async fn fetch_details(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    resource_type: ResourceType,
    identifier: &str,
) -> Result<Value> {
    let meta_host = metadata_url(host);
    let identifier = identifier.trim().to_lowercase();

    let unique_ids = if identifier.contains('.') {
        vec![identifier.clone()]
    } else {
        resolve_unique_ids(client, &meta_host, environment_id, resource_type, &identifier).await?
    };

    if unique_ids.is_empty() {
        return Err(DbtpError::graphql(format!(
            "Could not resolve name '{identifier}' to a unique_id"
        )));
    }

    let query = resource_type.query();
    let vars = json!({
        "environmentId": environment_id,
        "filter": {
            "uniqueIds": unique_ids,
            "types": [resource_type.graphql_type_name()],
        },
        "first": unique_ids.len(),
    });

    let data = client
        .discovery(&meta_host, environment_id, query, Some(vars))
        .await?;

    let edges = &data["environment"]["applied"]["resources"]["edges"];
    let nodes = extract_nodes(edges);

    if nodes.is_empty() {
        return Err(DbtpError::graphql(format!(
            "{} '{}' not found",
            resource_type.graphql_type_name(),
            identifier
        )));
    }

    if nodes.len() == 1 {
        Ok(nodes.into_iter().next().unwrap())
    } else {
        Ok(Value::Array(nodes))
    }
}

/// Resolve a plain name to candidate unique_ids by querying packages.
async fn resolve_unique_ids(
    client: &GraphqlClient,
    meta_host: &str,
    environment_id: u64,
    resource_type: ResourceType,
    name: &str,
) -> Result<Vec<String>> {
    let macro_vars = json!({
        "resource": "macro",
        "environmentId": environment_id,
    });
    let model_vars = json!({
        "resource": "model",
        "environmentId": environment_id,
    });

    let (macro_result, model_result) = tokio::try_join!(
        client.discovery(meta_host, environment_id, GET_PACKAGES, Some(macro_vars)),
        client.discovery(meta_host, environment_id, GET_PACKAGES, Some(model_vars)),
    )?;

    let macro_packages = macro_result["environment"]["applied"]["packages"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let model_packages = model_result["environment"]["applied"]["packages"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let all_packages: Vec<&str> = macro_packages
        .iter()
        .chain(model_packages.iter())
        .filter_map(|v| v.as_str())
        .collect();

    if all_packages.is_empty() {
        return Err(DbtpError::graphql("No packages found for this environment"));
    }

    let prefix = resource_type.id_prefix();
    let unique_ids: Vec<String> = all_packages
        .into_iter()
        .map(|pkg| format!("{prefix}.{pkg}.{name}"))
        .collect();

    Ok(unique_ids)
}
