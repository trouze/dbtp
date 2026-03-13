use serde_json::Value;
use tabled::{Table, settings::Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
    Compact,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "compact" => Self::Compact,
            _ => Self::Table,
        }
    }
}

pub fn format_output(value: &Value, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(value).unwrap_or_default(),
        OutputFormat::Yaml => serde_yaml::to_string(value).unwrap_or_default(),
        OutputFormat::Compact => serde_json::to_string(value).unwrap_or_default(),
        OutputFormat::Table => format_table(value),
    }
}

fn format_table(value: &Value) -> String {
    match value {
        Value::Array(arr) if !arr.is_empty() => format_array_table(arr),
        Value::Object(_) => format_object_table(value),
        _ => serde_json::to_string_pretty(value).unwrap_or_default(),
    }
}

fn format_array_table(arr: &[Value]) -> String {
    let Some(first) = arr.first() else {
        return String::new();
    };
    let Some(obj) = first.as_object() else {
        return serde_json::to_string_pretty(arr).unwrap_or_default();
    };

    let headers: Vec<String> = obj.keys().cloned().collect();

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(arr.len());
    for item in arr {
        let row: Vec<String> = headers
            .iter()
            .map(|h| value_to_cell(item.get(h).unwrap_or(&Value::Null)))
            .collect();
        rows.push(row);
    }

    let mut builder = tabled::builder::Builder::default();
    builder.push_record(&headers);
    for row in &rows {
        builder.push_record(row);
    }

    let mut table = builder.build();
    table.with(Style::rounded());
    table.to_string()
}

fn format_object_table(value: &Value) -> String {
    let Some(obj) = value.as_object() else {
        return String::new();
    };

    let mut builder = tabled::builder::Builder::default();
    builder.push_record(["Field", "Value"]);
    for (k, v) in obj {
        builder.push_record([k.as_str(), &value_to_cell(v)]);
    }

    let mut table = builder.build();
    table.with(Style::rounded());
    table.to_string()
}

fn value_to_cell(v: &Value) -> String {
    match v {
        Value::Null => "—".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(arr) => {
            if arr.len() <= 3 {
                format!("{}", serde_json::to_string(arr).unwrap_or_default())
            } else {
                format!("[{} items]", arr.len())
            }
        }
        Value::Object(_) => "{...}".into(),
    }
}
