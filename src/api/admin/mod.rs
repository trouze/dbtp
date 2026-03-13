pub mod accounts;
pub mod artifacts;
pub mod environments;
pub mod jobs;
pub mod projects;
pub mod runs;

use serde_json::{Map, Value};

const JOBS_COMPACT_FIELDS: &[&str] = &[
    "id",
    "name",
    "description",
    "dbt_version",
    "job_type",
    "triggers",
    "most_recent_run_id",
    "most_recent_run_status",
    "most_recent_run_started_at",
    "most_recent_run_finished_at",
    "most_recent_completed_run_id",
    "most_recent_completed_run_status",
    "most_recent_completed_run_started_at",
    "most_recent_completed_run_finished_at",
    "schedule",
    "next_run",
];

const RUNS_STRIP_FIELDS: &[&str] = &[
    "account_id",
    "environment_id",
    "blocked_by",
    "used_repo_cache",
    "audit",
    "created_at_humanized",
    "duration_humanized",
    "finished_at_humanized",
    "queued_duration_humanized",
    "run_duration_humanized",
    "artifacts_saved",
    "artifact_s3_path",
    "has_docs_generated",
    "has_sources_generated",
    "notifications_sent",
    "executed_by_thread_id",
    "updated_at",
    "dequeued_at",
    "last_checked_at",
    "last_heartbeat_at",
    "trigger",
    "run_steps",
    "deprecation",
    "environment",
];

const PROJECT_STRIP_FIELDS: &[&str] = &["freshness_job", "docs_job", "group_permissions"];

pub fn compact_job(val: &Value) -> Value {
    keep_fields(val, JOBS_COMPACT_FIELDS)
}

pub fn compact_jobs(val: &Value) -> Value {
    map_array(val, compact_job)
}

pub fn compact_run(val: &Value) -> Value {
    let Some(obj) = val.as_object() else {
        return val.clone();
    };
    let mut out = obj.clone();

    if let Some(job) = out.remove("job") {
        if let Some(name) = job.get("name") {
            out.insert("job_name".into(), name.clone());
        }
        if let Some(steps) = job.get("execute_steps") {
            out.insert("job_steps".into(), steps.clone());
        }
    }

    for &field in RUNS_STRIP_FIELDS {
        out.remove(field);
    }

    Value::Object(out)
}

pub fn compact_runs(val: &Value) -> Value {
    map_array(val, compact_run)
}

pub fn compact_project(val: &Value) -> Value {
    strip_fields(val, PROJECT_STRIP_FIELDS)
}

pub fn compact_artifacts(val: &Value) -> Value {
    let Value::Array(arr) = val else {
        return val.clone();
    };
    let filtered: Vec<Value> = arr
        .iter()
        .filter(|v| {
            let s = v.as_str().unwrap_or("");
            !s.starts_with("compiled/") && !s.starts_with("run/")
        })
        .cloned()
        .collect();
    Value::Array(filtered)
}

fn keep_fields(val: &Value, fields: &[&str]) -> Value {
    let Some(obj) = val.as_object() else {
        return val.clone();
    };
    let filtered: Map<String, Value> = obj
        .iter()
        .filter(|(k, _)| fields.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Value::Object(filtered)
}

fn strip_fields(val: &Value, fields: &[&str]) -> Value {
    let Some(obj) = val.as_object() else {
        return val.clone();
    };
    let filtered: Map<String, Value> = obj
        .iter()
        .filter(|(k, _)| !fields.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Value::Object(filtered)
}

fn map_array(val: &Value, f: fn(&Value) -> Value) -> Value {
    match val {
        Value::Array(arr) => Value::Array(arr.iter().map(f).collect()),
        other => f(other),
    }
}
