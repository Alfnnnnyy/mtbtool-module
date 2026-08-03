use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;

use crate::mtb::ensure_data_dir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub manual_lte: Vec<i32>,
    #[serde(rename = "manual_nrNsa", default)]
    pub manual_nr_nsa: Vec<i32>,
    #[serde(rename = "manual_nrSa", default)]
    pub manual_nr_sa: Vec<i32>,
    #[serde(default)]
    pub slot: i32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            manual_lte: Vec::new(),
            manual_nr_nsa: Vec::new(),
            manual_nr_sa: Vec::new(),
            slot: 0,
        }
    }
}

pub fn get_config() -> Value {
    let dir = match ensure_data_dir() {
        Ok(d) => d,
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };

    let config_path = dir.join("config.json");
    let cfg: Config = if config_path.exists() {
        fs::read_to_string(&config_path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    } else {
        Config::default()
    };

    json!({
        "ok": true,
        "config": cfg
    })
}

pub fn set_config(json_str: &str) -> Value {
    let cfg: Config = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => return json!({ "ok": false, "error": format!("Invalid config JSON: {}", e) }),
    };

    let dir = match ensure_data_dir() {
        Ok(d) => d,
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };

    let config_path = dir.join("config.json");
    let content = match serde_json::to_string_pretty(&cfg) {
        Ok(c) => c,
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };

    if let Err(e) = fs::write(&config_path, content) {
        return json!({ "ok": false, "error": format!("Failed to write config file: {}", e) });
    }

    json!({ "ok": true })
}
