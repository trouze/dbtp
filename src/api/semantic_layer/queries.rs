use std::time::Duration;

use arrow::array::*;
use arrow::datatypes::{DataType, TimeUnit};
use arrow::ipc::reader::StreamReader;
use base64::Engine;
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::core::error::{DbtpError, Result};
use crate::core::graphql_client::GraphqlClient;

use super::types::*;

const GET_SAVED_QUERIES: &str = include_str!("queries/get_saved_queries.graphql");
const CREATE_QUERY: &str = include_str!("queries/create_query.graphql");
const CREATE_DIMENSION_VALUES_QUERY: &str =
    include_str!("queries/create_dimension_values_query.graphql");
const GET_RESULTS: &str = include_str!("queries/get_results.graphql");

pub async fn list_saved_queries(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    search: Option<&str>,
) -> Result<Value> {
    let vars = json!({
        "environmentId": environment_id,
        "search": search,
    });
    let data = client
        .semantic_layer(host, environment_id, GET_SAVED_QUERIES, Some(vars))
        .await?;
    Ok(data
        .get("savedQueriesPaginated")
        .and_then(|v| v.get("items"))
        .cloned()
        .unwrap_or(Value::Array(vec![])))
}

pub async fn execute_query(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    metrics: &[String],
    group_by: &[GroupByInput],
    where_filters: &[WhereInput],
    order_by: &[OrderByInput],
    limit: Option<i64>,
) -> Result<Value> {
    let metric_inputs: Vec<Value> = metrics.iter().map(|n| json!({"name": n})).collect();
    let group_by_vals: Vec<Value> = group_by
        .iter()
        .map(|g| serde_json::to_value(g).unwrap_or_default())
        .collect();
    let where_vals: Vec<Value> = where_filters
        .iter()
        .map(|w| json!({"sql": w.sql}))
        .collect();
    let order_vals: Vec<Value> = order_by
        .iter()
        .map(|o| serde_json::to_value(o).unwrap_or_default())
        .collect();

    let vars = json!({
        "environmentId": environment_id,
        "metrics": metric_inputs,
        "groupBy": if group_by_vals.is_empty() { Value::Null } else { Value::Array(group_by_vals) },
        "where": if where_vals.is_empty() { Value::Null } else { Value::Array(where_vals) },
        "orderBy": if order_vals.is_empty() { Value::Null } else { Value::Array(order_vals) },
        "limit": limit,
    });

    let data = client
        .semantic_layer(host, environment_id, CREATE_QUERY, Some(vars))
        .await?;

    let query_id = data["createQuery"]["queryId"]
        .as_str()
        .ok_or_else(|| DbtpError::graphql("No queryId returned from CreateQuery mutation"))?;

    let pages = poll_for_results(client, host, environment_id, query_id).await?;

    let mut all_rows = Vec::new();
    let mut sql = None;
    for page in &pages {
        if let Some(s) = &page.sql {
            sql = Some(s.clone());
        }
        if let Some(arrow_data) = &page.arrow_result {
            let rows = decode_arrow_to_json(arrow_data)?;
            all_rows.extend(rows);
        }
    }

    let row_count = all_rows.len();
    Ok(json!({
        "rows": all_rows,
        "sql": sql,
        "row_count": row_count,
    }))
}

pub async fn compile_sql(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    metrics: &[String],
    group_by: &[GroupByInput],
    where_filters: &[WhereInput],
    order_by: &[OrderByInput],
    limit: Option<i64>,
) -> Result<Value> {
    let result = execute_query(
        client,
        host,
        environment_id,
        metrics,
        group_by,
        where_filters,
        order_by,
        limit,
    )
    .await?;
    Ok(json!({
        "sql": result.get("sql").cloned().unwrap_or(Value::Null),
    }))
}

pub async fn list_dimension_values(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    metrics: &[String],
    group_by: &[String],
) -> Result<Value> {
    let metric_inputs: Vec<Value> = metrics.iter().map(|n| json!({"name": n})).collect();
    let groupby_inputs: Vec<Value> = group_by.iter().map(|g| json!({"name": g})).collect();

    let vars = json!({
        "environmentId": environment_id,
        "metrics": metric_inputs,
        "groupBy": groupby_inputs,
    });

    let data = client
        .semantic_layer(host, environment_id, CREATE_DIMENSION_VALUES_QUERY, Some(vars))
        .await?;

    let query_id = data["createDimensionValuesQuery"]["queryId"]
        .as_str()
        .ok_or_else(|| {
            DbtpError::graphql("No queryId returned from CreateDimensionValuesQuery mutation")
        })?;

    let pages = poll_for_results(client, host, environment_id, query_id).await?;

    let mut all_rows = Vec::new();
    for page in &pages {
        if let Some(arrow_data) = &page.arrow_result {
            let rows = decode_arrow_to_json(arrow_data)?;
            all_rows.extend(rows);
        }
    }

    Ok(Value::Array(all_rows))
}

async fn poll_for_results(
    client: &GraphqlClient,
    host: &str,
    environment_id: u64,
    query_id: &str,
) -> Result<Vec<QueryPage>> {
    let mut page_num: i64 = 1;
    let mut pages = Vec::new();

    loop {
        let vars = json!({
            "environmentId": environment_id,
            "queryId": query_id,
            "pageNum": page_num,
        });

        let data = client
            .semantic_layer(host, environment_id, GET_RESULTS, Some(vars))
            .await?;

        let query_data = data
            .get("query")
            .ok_or_else(|| DbtpError::graphql("Missing 'query' field in GetResults response"))?;

        let page: QueryPage = serde_json::from_value(query_data.clone())?;

        match page.status.to_lowercase().as_str() {
            "successful" => {
                let total_pages = page.total_pages.unwrap_or(1);
                pages.push(page);
                if total_pages > page_num {
                    page_num += 1;
                } else {
                    break;
                }
            }
            "failed" => {
                let msg = page
                    .error
                    .unwrap_or_else(|| "Query execution failed".to_string());
                return Err(DbtpError::graphql(msg));
            }
            _ => {
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Ok(pages)
}

fn decode_arrow_to_json(arrow_result: &str) -> Result<Vec<Value>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(arrow_result)
        .map_err(|e| DbtpError::Arrow(format!("base64 decode: {e}")))?;

    let cursor = std::io::Cursor::new(bytes);
    let reader = StreamReader::try_new(cursor, None)
        .map_err(|e| DbtpError::Arrow(format!("Arrow IPC: {e}")))?;

    let schema = reader.schema().clone();
    let batches: Vec<RecordBatch> = reader
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| DbtpError::Arrow(format!("Arrow batch read: {e}")))?;

    let mut rows = Vec::new();
    for batch in &batches {
        for row in 0..batch.num_rows() {
            let mut map = serde_json::Map::new();
            for (col_idx, field) in schema.fields().iter().enumerate() {
                let col = batch.column(col_idx);
                let val = arrow_value_to_json(col.as_ref(), row);
                map.insert(field.name().clone(), val);
            }
            rows.push(Value::Object(map));
        }
    }

    Ok(rows)
}

fn arrow_value_to_json(col: &dyn Array, row: usize) -> Value {
    if col.is_null(row) {
        return Value::Null;
    }

    if let Some(arr) = col.as_any().downcast_ref::<StringArray>() {
        return Value::String(arr.value(row).to_string());
    }
    if let Some(arr) = col.as_any().downcast_ref::<LargeStringArray>() {
        return Value::String(arr.value(row).to_string());
    }
    if let Some(arr) = col.as_any().downcast_ref::<BooleanArray>() {
        return Value::Bool(arr.value(row));
    }
    if let Some(arr) = col.as_any().downcast_ref::<Int8Array>() {
        return Value::Number(arr.value(row).into());
    }
    if let Some(arr) = col.as_any().downcast_ref::<Int16Array>() {
        return Value::Number(arr.value(row).into());
    }
    if let Some(arr) = col.as_any().downcast_ref::<Int32Array>() {
        return Value::Number(arr.value(row).into());
    }
    if let Some(arr) = col.as_any().downcast_ref::<Int64Array>() {
        return Value::Number(arr.value(row).into());
    }
    if let Some(arr) = col.as_any().downcast_ref::<UInt8Array>() {
        return Value::Number(arr.value(row).into());
    }
    if let Some(arr) = col.as_any().downcast_ref::<UInt16Array>() {
        return Value::Number(arr.value(row).into());
    }
    if let Some(arr) = col.as_any().downcast_ref::<UInt32Array>() {
        return Value::Number(arr.value(row).into());
    }
    if let Some(arr) = col.as_any().downcast_ref::<UInt64Array>() {
        return Value::Number(arr.value(row).into());
    }
    if let Some(arr) = col.as_any().downcast_ref::<Float32Array>() {
        return serde_json::Number::from_f64(arr.value(row) as f64)
            .map(Value::Number)
            .unwrap_or(Value::Null);
    }
    if let Some(arr) = col.as_any().downcast_ref::<Float64Array>() {
        return serde_json::Number::from_f64(arr.value(row))
            .map(Value::Number)
            .unwrap_or(Value::Null);
    }
    if let Some(arr) = col.as_any().downcast_ref::<Decimal128Array>() {
        let v = arr.value(row);
        if let DataType::Decimal128(_, scale) = col.data_type() {
            if *scale == 0 {
                return Value::Number(serde_json::Number::from(v as i64));
            }
            let divisor = 10_f64.powi(*scale as i32);
            return serde_json::Number::from_f64(v as f64 / divisor)
                .map(Value::Number)
                .unwrap_or(Value::Null);
        }
    }
    if let Some(arr) = col.as_any().downcast_ref::<Date32Array>() {
        let days = arr.value(row);
        let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        return chrono::TimeDelta::try_days(days as i64)
            .and_then(|delta| epoch.checked_add_signed(delta))
            .map(|d| Value::String(d.to_string()))
            .unwrap_or(Value::Null);
    }
    if let Some(arr) = col.as_any().downcast_ref::<Date64Array>() {
        let ms = arr.value(row);
        return chrono::DateTime::from_timestamp_millis(ms)
            .map(|d| Value::String(d.format("%Y-%m-%d").to_string()))
            .unwrap_or(Value::Null);
    }

    match col.data_type() {
        DataType::Timestamp(TimeUnit::Second, _) => {
            let arr = col
                .as_any()
                .downcast_ref::<TimestampSecondArray>()
                .unwrap();
            chrono::DateTime::from_timestamp(arr.value(row), 0)
                .map(|d| Value::String(d.to_rfc3339()))
                .unwrap_or(Value::Null)
        }
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            let arr = col
                .as_any()
                .downcast_ref::<TimestampMillisecondArray>()
                .unwrap();
            chrono::DateTime::from_timestamp_millis(arr.value(row))
                .map(|d| Value::String(d.to_rfc3339()))
                .unwrap_or(Value::Null)
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            let arr = col
                .as_any()
                .downcast_ref::<TimestampMicrosecondArray>()
                .unwrap();
            chrono::DateTime::from_timestamp_micros(arr.value(row))
                .map(|d| Value::String(d.to_rfc3339()))
                .unwrap_or(Value::Null)
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            let arr = col
                .as_any()
                .downcast_ref::<TimestampNanosecondArray>()
                .unwrap();
            let v = arr.value(row);
            chrono::DateTime::from_timestamp(v / 1_000_000_000, (v % 1_000_000_000) as u32)
                .map(|d| Value::String(d.to_rfc3339()))
                .unwrap_or(Value::Null)
        }
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Float64Array, Int64Array, RecordBatch, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::ipc::writer::StreamWriter;
    use base64::Engine;
    use std::sync::Arc;

    fn make_test_arrow_base64() -> String {
        let schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new("value", DataType::Float64, false),
            Field::new("count", DataType::Int64, false),
        ]));

        let names = StringArray::from(vec!["revenue", "orders", "aov"]);
        let values = Float64Array::from(vec![1234.56, 42.0, 29.42]);
        let counts = Int64Array::from(vec![100, 200, 300]);

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(names), Arc::new(values), Arc::new(counts)],
        )
        .unwrap();

        let mut buf = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut buf, &schema).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }

        base64::engine::general_purpose::STANDARD.encode(&buf)
    }

    #[test]
    fn test_decode_arrow_to_json() {
        let encoded = make_test_arrow_base64();
        let rows = decode_arrow_to_json(&encoded).unwrap();

        assert_eq!(rows.len(), 3);

        assert_eq!(rows[0]["name"], "revenue");
        assert_eq!(rows[0]["value"], 1234.56);
        assert_eq!(rows[0]["count"], 100);

        assert_eq!(rows[1]["name"], "orders");
        assert_eq!(rows[1]["value"], 42.0);
        assert_eq!(rows[1]["count"], 200);

        assert_eq!(rows[2]["name"], "aov");
        assert_eq!(rows[2]["value"], 29.42);
        assert_eq!(rows[2]["count"], 300);
    }

    #[test]
    fn test_decode_arrow_empty() {
        let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Utf8, false)]));

        let mut buf = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut buf, &schema).unwrap();
            writer.finish().unwrap();
        }

        let encoded = base64::engine::general_purpose::STANDARD.encode(&buf);
        let rows = decode_arrow_to_json(&encoded).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_decode_arrow_with_nulls() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, true),
            Field::new("value", DataType::Float64, true),
        ]));

        let names = StringArray::from(vec![Some("a"), None, Some("c")]);
        let values = Float64Array::from(vec![Some(1.0), Some(2.0), None]);

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(names), Arc::new(values)],
        )
        .unwrap();

        let mut buf = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut buf, &schema).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }

        let encoded = base64::engine::general_purpose::STANDARD.encode(&buf);
        let rows = decode_arrow_to_json(&encoded).unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["name"], "a");
        assert_eq!(rows[0]["value"], 1.0);
        assert!(rows[1]["name"].is_null());
        assert_eq!(rows[1]["value"], 2.0);
        assert_eq!(rows[2]["name"], "c");
        assert!(rows[2]["value"].is_null());
    }
}
