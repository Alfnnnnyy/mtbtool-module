//! RPC-only dispatch layer (security hardening).
//!
//! The WebUI may execute EXACTLY ONE fixed command:
//!   mtbctl rpc --b64 <base64url-payload>
//! The payload is a JSON object {"method","params"}. The method MUST match an
//! explicit allowlist; params are validated by the handlers. No shell
//! interpolation ever happens here — /vendor/bin/mtb is invoked with
//! Command::new and one argv element per argument.

use serde_json::{json, Value};

use crate::backup::{list_backups, restore_backup};
use crate::bandlock::{detect_bandlock, get_bandlock, set_bandlock};
use crate::cells::get_cells;
use crate::config::{get_config, set_config};
use crate::features::{check_features, disable_feature, restore_feature};
use crate::importer::{import_apply, import_preview};
use crate::mtb::exec_mtb;
use crate::nv::{delete_nv, read_nv, write_nv};

/// Absolute cap on a single RPC payload (argv size safety).
const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

const ALLOWED_METHODS: &[&str] = &[
    "probe",
    "nv.read",
    "nv.write",
    "nv.delete",
    "bandlock.get",
    "bandlock.set",
    "bandlock.detect",
    "features.check",
    "features.disable",
    "features.restore",
    "cells.get",
    "modem.restart",
    "import.preview",
    "import.apply",
    "backup.list",
    "backup.restore",
    "config.get",
    "config.set",
];

pub fn is_allowed_method(method: &str) -> bool {
    ALLOWED_METHODS.contains(&method)
}

/// Decode base64url (RFC 4648 §5, padding optional, charset A-Za-z0-9_-).
pub fn decode_base64url(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4 + 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for c in input.chars() {
        let v = match c {
            'A'..='Z' => (c as u32) - ('A' as u32),
            'a'..='z' => (c as u32) - ('a' as u32) + 26,
            '0'..='9' => (c as u32) - ('0' as u32) + 52,
            '-' => 62,
            '_' => 63,
            '=' => break, // padding: ignore the rest
            _ => return Err(format!("invalid base64url character: {:?}", c)),
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    if bits > 0 && bits < 8 {
        // leftover bits must be zero-padded; a trailing partial group is
        // only legal if the remaining bits are zero (canonical encoding)
        let rem = acc & ((1 << bits) - 1);
        if rem != 0 {
            return Err("invalid base64url: nonzero trailing bits".to_string());
        }
    }
    Ok(out)
}

/// Handle `mtbctl rpc --b64 <payload>`: decode, validate, dispatch.
pub fn rpc_exec(payload: &str) -> Value {
    let decoded = match decode_base64url(payload) {
        Ok(b) => b,
        Err(e) => return json!({ "ok": false, "error": format!("rpc decode: {}", e) }),
    };
    if decoded.len() > MAX_PAYLOAD_BYTES {
        return json!({ "ok": false, "error": "rpc payload exceeds 64 KiB limit" });
    }
    let text = match String::from_utf8(decoded) {
        Ok(t) => t,
        Err(_) => return json!({ "ok": false, "error": "rpc payload is not UTF-8" }),
    };
    let req: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": format!("rpc payload is not JSON: {}", e) }),
    };
    let method = req
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !is_allowed_method(method) {
        return json!({ "ok": false, "error": format!("Unknown RPC method: {}", method) });
    }
    let params = req.get("params").cloned().unwrap_or_else(|| json!({}));
    if !params.is_object() {
        return json!({ "ok": false, "error": "rpc params must be an object" });
    }
    dispatch(method, &params)
}

/// Dispatch a validated method+params to its handler. Accepts both the
/// canonical dot form ("nv.read") and the legacy space form ("nv read").
pub fn dispatch(method: &str, params: &Value) -> Value {
    let canonical = method.replace('.', " ");
    let parts: Vec<&str> = canonical.split_whitespace().collect();
    let base = parts.get(0..2).map(|s| s.join(" ")).unwrap_or_else(|| canonical.clone());

    // Frontend sends slot as number or string — accept both.
    let slot = params
        .get("slot")
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())))
        .map(|v| v as i32)
        .unwrap_or(0);

    let str_arg = |key: &str| params.get(key).and_then(|v| v.as_str());

    match base.as_str() {
        "probe" => {
            let (code, raw) = exec_mtb(&["0"]);
            let mtb_bin = crate::mtb::get_mtb_bin();
            let mtb_exists = std::path::Path::new(&mtb_bin).exists();
            let mtb_executable = mtb_exists && code == 0;
            let mtb_responds = mtb_exists && (!raw.trim().is_empty() || code == 0);
            let ok = mtb_exists && mtb_executable && mtb_responds;
            let model = crate::util::getprop("ro.product.model");
            let device = crate::util::getprop("ro.product.device");
            let sdk = crate::util::getprop("ro.build.version.sdk");
            json!({
                "ok": ok,
                "mtb_path": mtb_bin,
                "mtb_exists": mtb_exists,
                "mtb_executable": mtb_executable,
                "mtb_responds": mtb_responds,
                "mtbctl_version": env!("CARGO_PKG_VERSION"),
                "model": model,
                "device": device,
                "android_sdk": sdk,
                "data_dir": crate::mtb::get_mtbtool_dir().to_string_lossy()
            })
        }
        "nv read" => read_nv(str_arg("path").unwrap_or(""), slot),
        "nv write" => write_nv(
            str_arg("path").unwrap_or(""),
            str_arg("hex").unwrap_or(""),
            slot,
            str_arg("reason"),
        ),
        "nv delete" => delete_nv(str_arg("path").unwrap_or(""), slot, str_arg("reason")),
        "bandlock get" => get_bandlock(slot),
        "bandlock set" => {
            let allow_empty = params
                .get("allowEmpty")
                .map(|v| v.as_bool().unwrap_or_else(|| v.as_str() == Some("true")))
                .unwrap_or(false);
            set_bandlock(
                slot,
                str_arg("lte"),
                str_arg("nrNsa"),
                str_arg("nrSa"),
                allow_empty,
            )
        }
        "bandlock detect" => detect_bandlock(slot),
        "features check" => check_features(slot),
        "features disable" => disable_feature(str_arg("id").unwrap_or(""), slot),
        "features restore" => restore_feature(str_arg("id").unwrap_or(""), slot),
        "cells get" => get_cells(slot),
        "modem restart" => {
            let (code, _) = exec_mtb(&["11", "0"]);
            json!({ "ok": code == 0, "exit": code })
        }
        "import preview" => {
            let json_str = str_arg("json").unwrap_or("");
            import_preview(json_str)
        }
        "import apply" => {
            let json_str = str_arg("json").unwrap_or("");
            import_apply(json_str)
        }
        "backup list" => match list_backups() {
            Ok(backups) => json!({ "ok": true, "backups": backups }),
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "backup restore" => {
            let id = str_arg("id")
                .or_else(|| parts.get(2).copied())
                .unwrap_or("latest");
            match restore_backup(id) {
                Ok(restored) => {
                    let overall_ok = restored
                        .iter()
                        .all(|r| r["ok"] == true && r["verified"] == true);
                    json!({ "ok": overall_ok, "restored": restored })
                }
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }
        "config get" => get_config(),
        "config set" => set_config(str_arg("json").unwrap_or("")),
        _ => json!({ "ok": false, "error": format!("Unknown command: {}", method) }),
    }
}
