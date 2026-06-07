//! Config file at `~/.config/mnml-db-dynamodb.toml`. First run
//! writes the scaffold + exits with instructions.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Optional default region — overridden per-tab via `region`.
    #[serde(default)]
    pub region: Option<String>,
    /// Polling interval. `0` disables auto-refresh.
    #[serde(default = "default_refresh")]
    pub refresh_interval_secs: u64,
    /// Tab list — at least one required.
    #[serde(default)]
    pub tabs: Vec<Tab>,
}

fn default_refresh() -> u64 {
    0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub name: String,
    /// DynamoDB table name. Required.
    pub table: String,
    /// Max items to fetch per scan. Default 50.
    #[serde(default = "default_scan_limit")]
    pub scan_limit: u32,
    #[serde(default)]
    pub region: Option<String>,
}

fn default_scan_limit() -> u32 {
    50
}

impl Config {
    pub const EXAMPLE: &'static str = r##"# mnml-db-dynamodb config. Edit and re-run.
#
# Optional top-level region (defers to AWS CLI when unset):
# region = "us-east-1"

refresh_interval_secs = 0

# ── Tabs ─────────────────────────────────────────────────────────
# Each [[tabs]] entry is one DynamoDB table.

[[tabs]]
name = "Sessions"
table = "user-sessions"
scan_limit = 50

[[tabs]]
name = "Orders"
table = "orders"
scan_limit = 100

[[tabs]]
name = "Events"
table = "domain-events"
"##;

    pub fn validate(&self) -> Result<()> {
        if self.tabs.is_empty() {
            return Err(anyhow!("config: at least one [[tabs]] entry required"));
        }
        for (i, t) in self.tabs.iter().enumerate() {
            if t.table.trim().is_empty() {
                return Err(anyhow!("tab #{i} ({}): `table` is required", t.name));
            }
            if t.scan_limit == 0 || t.scan_limit > 1000 {
                return Err(anyhow!(
                    "tab #{i} ({}): scan_limit must be 1..=1000 (got {})",
                    t.name,
                    t.scan_limit
                ));
            }
        }
        Ok(())
    }
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("mnml-db-dynamodb.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, Config::EXAMPLE)?;
        return Err(anyhow!(
            "wrote config template to {} — edit it then re-run",
            path.display()
        ));
    }
    let text = std::fs::read_to_string(&path)?;
    let cfg: Config = toml::from_str(&text)?;
    cfg.validate()?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_parses_and_validates() {
        let cfg: Config = toml::from_str(Config::EXAMPLE).expect("parses");
        cfg.validate().expect("validates");
    }

    #[test]
    fn rejects_empty_table() {
        let raw = r##"
[[tabs]]
name = "bad"
table = ""
"##;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn rejects_zero_scan_limit() {
        let raw = r##"
[[tabs]]
name = "bad"
table = "x"
scan_limit = 0
"##;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn rejects_huge_scan_limit() {
        let raw = r##"
[[tabs]]
name = "bad"
table = "x"
scan_limit = 9999
"##;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert!(cfg.validate().is_err());
    }
}
