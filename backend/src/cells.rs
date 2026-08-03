use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::mtb::exec_mtb;
use crate::util::validate_slot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LteCellData {
    pub label: String,
    pub earfcn: i32,
    pub pci: i32,
    pub rsrp: f32,
    pub rsrq: f32,
    pub rssi: f32,
    pub snr: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NrCellData {
    pub label: String,
    pub rsrp: f32,
    pub rsrq: f32,
}

pub fn parse_asdiv_line(raw: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("ASDIV DATA:") {
            let content = &trimmed["ASDIV DATA:".len()..].trim();
            for pair in content.split(", ") {
                if let Some(idx) = pair.find(": ") {
                    let k = pair[..idx].trim().to_string();
                    let v = pair[idx + 2..].trim().to_string();
                    map.insert(k, v);
                }
            }
            break;
        }
    }
    map
}

fn is_invalid_float(v: f32) -> bool {
    v >= 65534.5
}

pub fn parse_lte_cell(raw: &str, label: &str) -> Option<LteCellData> {
    let map = parse_asdiv_line(raw);
    if map.is_empty() {
        return None;
    }

    let earfcn = map.get("earfcn")?.parse::<i32>().ok()?;
    let pci = map.get("pci")?.parse::<i32>().ok()?;
    let rsrp = map.get("rsrp_rx0")?.parse::<f32>().ok()?;
    let rsrq = map.get("rsrq_rx0")?.parse::<f32>().ok()?;
    let rssi = map.get("rssi_rx0")?.parse::<f32>().ok()?;
    let snr = map.get("snr_rx0")?.parse::<f32>().ok()?;

    if is_invalid_float(rsrp) || is_invalid_float(rsrq) || is_invalid_float(rssi) || is_invalid_float(snr) {
        return None;
    }

    Some(LteCellData {
        label: label.to_string(),
        earfcn,
        pci,
        rsrp,
        rsrq,
        rssi,
        snr,
    })
}

pub fn parse_nr_cell(raw: &str, label: &str) -> Option<NrCellData> {
    let map = parse_asdiv_line(raw);
    if map.is_empty() {
        return None;
    }

    let rsrp = map.get("rsrp_rx0")?.parse::<f32>().ok()?;
    let rsrq = map.get("rsrq")?.parse::<f32>().ok()?;

    if is_invalid_float(rsrp) || is_invalid_float(rsrq) {
        return None;
    }

    Some(NrCellData {
        label: label.to_string(),
        rsrp,
        rsrq,
    })
}

pub fn parse_tx_power(raw: &str) -> Option<i32> {
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("TX INFO:") {
            let content = &trimmed["TX INFO:".len()..].trim();
            for pair in content.split(", ") {
                if let Some(idx) = pair.find(" = ") {
                    let k = pair[..idx].trim();
                    let v = pair[idx + 3..].trim();
                    if k == "tx_power" {
                        if let Ok(val) = v.parse::<i32>() {
                            if val == 65535 {
                                return None;
                            } else {
                                return Some(val);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn get_cells(slot: i32) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let lte_opts = [
        (0, "PCC"),
        (1, "SCC1"),
        (2, "SCC2"),
        (3, "SCC3"),
    ];

    let mut lte_cells = Vec::new();
    for (opt, label) in lte_opts {
        let (_, raw) = exec_mtb(&["9", &opt.to_string(), &slot.to_string()]);
        if let Some(cell) = parse_lte_cell(&raw, label) {
            lte_cells.push(cell);
        }
    }

    let nr_opts = [
        (10, "PCC"),
        (11, "SCC1"),
        (12, "SCC2"),
    ];

    let mut nr_cells = Vec::new();
    for (opt, label) in nr_opts {
        let (_, raw) = exec_mtb(&["9", &opt.to_string(), &slot.to_string()]);
        if let Some(cell) = parse_nr_cell(&raw, label) {
            nr_cells.push(cell);
        }
    }

    let (_, tx_raw) = exec_mtb(&["9", "31", &slot.to_string()]);
    let tx_power = parse_tx_power(&tx_raw);

    json!({
        "ok": true,
        "ts": now,
        "lte": lte_cells,
        "nr": nr_cells,
        "tx_power": tx_power
    })
}
