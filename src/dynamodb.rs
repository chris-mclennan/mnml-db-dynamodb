//! Thin wrappers around `aws dynamodb scan` / `describe-table`.

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::Value;
use std::process::Command;
use std::sync::mpsc::{Receiver, channel};
use std::thread;

#[derive(Debug, Clone)]
pub struct Item {
    pub raw: Value,
    pub primary: String,
    pub secondary: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TableMeta {
    pub name: String,
    pub pk_field: Option<String>,
    pub sk_field: Option<String>,
    pub item_count: Option<u64>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum DynamoEvent {
    Scanned { items: Vec<Item>, meta: TableMeta },
    Failed(String),
}

pub fn spawn_scan(table: String, limit: u32, region: Option<String>) -> Receiver<DynamoEvent> {
    let (tx, rx) = channel();
    thread::spawn(move || {
        let result = scan(&table, limit, region.as_deref());
        let _ = match result {
            Ok((items, meta)) => tx.send(DynamoEvent::Scanned { items, meta }),
            Err(e) => tx.send(DynamoEvent::Failed(e.to_string())),
        };
    });
    rx
}

fn scan(table: &str, limit: u32, region: Option<&str>) -> Result<(Vec<Item>, TableMeta)> {
    let limit_s = limit.to_string();
    let scan_json = run_aws(
        &["dynamodb", "scan", "--table-name", table, "--limit", &limit_s],
        region,
    )?;
    let desc_json = run_aws(
        &["dynamodb", "describe-table", "--table-name", table],
        region,
    )
    .ok()
    .unwrap_or(Value::Null);

    let raw_items: ScanResponse = serde_json::from_value(scan_json)
        .context("parse dynamodb scan response")?;
    let meta = parse_meta(table, &desc_json);

    let items = raw_items
        .items
        .iter()
        .map(|raw| build_item(raw, &meta))
        .collect();
    Ok((items, meta))
}

fn build_item(raw_attr: &Value, meta: &TableMeta) -> Item {
    let primary = meta
        .pk_field
        .as_deref()
        .and_then(|pk| extract_attr(raw_attr, pk))
        .unwrap_or_else(|| extract_first_value(raw_attr));
    let primary = if let Some(sk) = meta.sk_field.as_deref()
        && let Some(sk_val) = extract_attr(raw_attr, sk)
    {
        format!("{primary} · {sk_val}")
    } else {
        primary
    };
    let secondary = compact_secondary(raw_attr, meta);
    Item {
        raw: raw_attr.clone(),
        primary,
        secondary,
    }
}

fn extract_attr(item: &Value, attr: &str) -> Option<String> {
    let inner = item.get(attr)?;
    if let Some(s) = inner.get("S").and_then(|v| v.as_str()) {
        Some(s.to_string())
    } else if let Some(n) = inner.get("N").and_then(|v| v.as_str()) {
        Some(n.to_string())
    } else if let Some(b) = inner.get("BOOL").and_then(|v| v.as_bool()) {
        Some(b.to_string())
    } else {
        None
    }
}

fn extract_first_value(item: &Value) -> String {
    if let Some(obj) = item.as_object() {
        for (k, _) in obj.iter().take(1) {
            if let Some(v) = extract_attr(item, k) {
                return format!("{k}={v}");
            }
        }
    }
    "(empty)".to_string()
}

fn compact_secondary(item: &Value, meta: &TableMeta) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(obj) = item.as_object() {
        for (k, _) in obj.iter() {
            if Some(k.as_str()) == meta.pk_field.as_deref() {
                continue;
            }
            if Some(k.as_str()) == meta.sk_field.as_deref() {
                continue;
            }
            if let Some(v) = extract_attr(item, k) {
                let v_short = if v.len() > 30 {
                    format!("{}…", &v[..30])
                } else {
                    v
                };
                parts.push(format!("{k}={v_short}"));
            }
            if parts.len() >= 4 {
                break;
            }
        }
    }
    parts.join("  ")
}

fn parse_meta(table: &str, desc: &Value) -> TableMeta {
    let mut pk_field: Option<String> = None;
    let mut sk_field: Option<String> = None;
    if let Some(key_schema) = desc.pointer("/Table/KeySchema").and_then(|v| v.as_array()) {
        for ks in key_schema {
            let name = ks
                .get("AttributeName")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let kind = ks.get("KeyType").and_then(|v| v.as_str()).unwrap_or("");
            match kind {
                "HASH" => pk_field = name,
                "RANGE" => sk_field = name,
                _ => {}
            }
        }
    }
    let item_count = desc.pointer("/Table/ItemCount").and_then(|v| v.as_u64());
    let size_bytes = desc
        .pointer("/Table/TableSizeBytes")
        .and_then(|v| v.as_u64());
    TableMeta {
        name: table.to_string(),
        pk_field,
        sk_field,
        item_count,
        size_bytes,
    }
}

pub fn console_url(table: &str, region: Option<&str>) -> String {
    let r = region.unwrap_or("us-east-1");
    format!(
        "https://{r}.console.aws.amazon.com/dynamodbv2/home?region={r}#item-explorer?table={table}"
    )
}

fn run_aws(args: &[&str], region: Option<&str>) -> Result<Value> {
    let mut cmd = Command::new("aws");
    if let Some(r) = region {
        cmd.arg("--region").arg(r);
    }
    cmd.args(args).arg("--output").arg("json");
    let out = cmd
        .output()
        .map_err(|e| anyhow!("spawn aws: {e} — is the AWS CLI on PATH?"))?;
    if !out.status.success() {
        return Err(anyhow!(
            "aws {} → {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    if out.stdout.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&out.stdout).map_err(|e| anyhow!("parse json: {e}"))
}

#[derive(Debug, Deserialize)]
struct ScanResponse {
    #[serde(rename = "Items", default)]
    items: Vec<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_attr_from_string() {
        let v = json!({ "name": { "S": "Alice" } });
        assert_eq!(extract_attr(&v, "name"), Some("Alice".to_string()));
    }

    #[test]
    fn extract_attr_from_number() {
        let v = json!({ "count": { "N": "42" } });
        assert_eq!(extract_attr(&v, "count"), Some("42".to_string()));
    }

    #[test]
    fn extract_attr_returns_none_for_missing_field() {
        let v = json!({ "name": { "S": "Alice" } });
        assert_eq!(extract_attr(&v, "age"), None);
    }

    #[test]
    fn parse_meta_extracts_keys() {
        let desc = json!({
            "Table": {
                "KeySchema": [
                    { "AttributeName": "userId", "KeyType": "HASH" },
                    { "AttributeName": "ts", "KeyType": "RANGE" }
                ],
                "ItemCount": 1234,
                "TableSizeBytes": 9876
            }
        });
        let meta = parse_meta("Sessions", &desc);
        assert_eq!(meta.pk_field.as_deref(), Some("userId"));
        assert_eq!(meta.sk_field.as_deref(), Some("ts"));
        assert_eq!(meta.item_count, Some(1234));
    }

    #[test]
    fn console_url_includes_region_and_table() {
        let url = console_url("Sessions", Some("us-west-2"));
        assert!(url.contains("us-west-2"));
        assert!(url.contains("table=Sessions"));
    }
}
