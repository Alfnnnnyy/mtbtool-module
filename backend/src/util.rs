pub fn validate_nv_path(path: &str) -> Result<(), String> {
    if path.contains("..") || path.chars().any(|c| c.is_control()) {
        return Err("Invalid path: contains '..' or control characters".to_string());
    }

    let allowed_prefixes = [
        "/nv/item_files/modem/mmode/",
        "/nv/item_files/modem/lte/rrc/efs/",
        "/nv/item_files/modem/nr5g/RRC/",
        "/nv/item_files/modem/lte/RRC/",
    ];

    if allowed_prefixes.iter().any(|prefix| path.starts_with(prefix)) {
        Ok(())
    } else {
        Err(format!("Path '{}' does not match allowed NV prefixes", path))
    }
}

pub fn validate_slot(slot: i32) -> Result<(), String> {
    if slot == 0 || slot == 1 {
        Ok(())
    } else {
        Err(format!("Invalid slot: {}. Must be 0 or 1", slot))
    }
}

pub fn validate_hex(hex_str: &str) -> Result<(), String> {
    if hex_str.len() % 2 != 0 {
        return Err("Hex string must have an even length".to_string());
    }
    if hex_str.len() > 1024 {
        return Err("Hex string length exceeds maximum of 1024 characters".to_string());
    }
    if !hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Hex string contains non-hex characters".to_string());
    }
    Ok(())
}
pub fn is_valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
pub fn perform_verified_rollback(
    slot: i32,
    before_states: &[(String, Option<Vec<u8>>)],
) -> serde_json::Value {
    let states: Vec<(i32, String, Option<Vec<u8>>)> = before_states
        .iter()
        .map(|(p, b)| (slot, p.clone(), b.clone()))
        .collect();
    perform_verified_rollback_entries(&states)
}

/// Per-entry variant used when before-states span multiple SIM slots
/// (e.g. backup.restore). Each entry carries its own slot.
pub fn perform_verified_rollback_entries(
    states: &[(i32, String, Option<Vec<u8>>)],
) -> serde_json::Value {
    let mut entries = Vec::new();
    let mut all_verified = true;

    for (slot, path, before) in states {
        let (action, exit) = match before {
            Some(b) => {
                let mut write_args: Vec<String> =
                    vec!["4".into(), "5".into(), slot.to_string(), path.clone()];
                write_args.extend(b.iter().map(|byte| byte.to_string()));
                let (code, _) = crate::mtb::exec_mtb_owned(write_args);
                ("write", code)
            }
            None => {
                let (code, _) = crate::mtb::exec_mtb(&["4", "6", &slot.to_string(), path]);
                ("delete", code)
            }
        };

        // Re-read to verify before-state restored
        let (exit_after, raw_after) = crate::mtb::exec_mtb(&["4", "4", &slot.to_string(), path]);
        let verified = match parse_efs_read_output(exit_after, &raw_after) {
            EfsRead::Present(bytes_after) => match before {
                Some(b) => bytes_after == *b,
                None => false,
            },
            EfsRead::Absent => before.is_none(),
            EfsRead::Error(_) => false,
        };

        if !verified {
            all_verified = false;
        }

        entries.push(serde_json::json!({
            "slot": slot,
            "path": path,
            "action": action,
            "exit": exit,
            "verified": verified
        }));
    }

    serde_json::json!({
        "attempted": true,
        "verified": all_verified,
        "entries": entries
    })
}

pub fn parse_hex(hex_str: &str) -> Result<Vec<u8>, String> {
    validate_hex(hex_str)?;
    (0..hex_str.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex_str[i..i + 2], 16)
                .map_err(|e| format!("Failed to parse hex at index {}: {}", i, e))
        })
        .collect()
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}


pub fn parse_space_dec(s: &str) -> Result<Vec<u8>, String> {
    s.split_whitespace()
        .map(|tok| {
            tok.parse::<u8>()
                .map_err(|_| format!("Invalid byte in space-dec string: '{}'", tok))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EfsRead {
    Present(Vec<u8>),
    Absent,
    Error(String),
}

/// Semantic failure markers emitted by the real /vendor/bin/mtb even when
/// the process exits 0 (verified on POCO F6 / peridot, Android 14):
/// `rsp.result = -117` QMI failures print "qmi response fail" / "... fail,
/// REQUEST_ID_EFS" / "Error happen! error_code(-117)" with exit code 0.
/// Exposed for DIAG responses too (semantic failures can come with exit 0).
pub fn has_diag_failure_marker(line: &str) -> bool {
    line.contains("rsp.result = -")
        || line.contains("qmi response fail")
        || line.contains("send_sync fail")
        || line.contains("Error happen!")
        || line.contains("error_code(")
}

const FAILURE_MARKERS: &[&str] = &[
    "rsp.result = -",
    "qmi response fail",
    "send_sync fail",
    "Error happen!",
    "error_code(",
];

/// Parse a real `mtb 4 4` EFS read response (POCO F6 / peridot format).
///
/// Real-world output shape (verified against on-device captures):
/// - each byte is printed on its own line as
///   `<prefix> ... xiaomi_nvefs_test_efs_read:  FF`
/// - the whole dump is printed TWICE: once with an `mtb:` prefix and once
///   with an `RIL` prefix (the RIL block is the authoritative, complete one —
///   the mtb block can be truncated, e.g. 63 of 64 bytes)
/// - a `data len(N)` line declares the expected byte count
/// - QMI semantic failures print FAILURE_MARKERS with exit code 0
/// - stream interleaving can merge two lines; prefixes stay intact on the
///   vast majority of lines, so classification is by line prefix
pub fn parse_efs_read_output(exit_code: i32, raw: &str) -> EfsRead {
    if exit_code != 0 {
        return EfsRead::Error(format!("exit {}", exit_code));
    }
    if raw.lines().any(has_diag_failure_marker) {
        return EfsRead::Error("qmi read failure reported by mtb".to_string());
    }

    let mut declared: Option<usize> = None;
    let mut mtb: Vec<u8> = Vec::new();
    let mut ril: Vec<u8> = Vec::new();
    let mut stray: Vec<u8> = Vec::new();

    for line in raw.lines() {
        if !line.contains("xiaomi_nvefs_test_efs_read:") {
            continue;
        }
        if let Some(pos) = line.find("data len(") {
            if let Some(rest) = line[pos + "data len(".len()..].split(')').next() {
                if let Ok(n) = rest.trim().parse::<usize>() {
                    declared = Some(n);
                }
            }
        }
        let last = line.split_whitespace().last().unwrap_or("");
        if last.len() == 2 && last.chars().all(|c| c.is_ascii_hexdigit()) {
            if let Ok(b) = u8::from_str_radix(last, 16) {
                if line.starts_with("mtb:") {
                    mtb.push(b);
                } else if line.starts_with("RIL") {
                    ril.push(b);
                } else {
                    stray.push(b);
                }
            }
        }
    }

    let n = declared.unwrap_or(0);
    if n == 0 {
        // No declared length: an empty response means the item is absent
        // (modem default). Any bytes without a declared length are
        // unexpected — surface as an error rather than guessing.
        if mtb.is_empty() && ril.is_empty() && stray.is_empty() {
            EfsRead::Absent
        } else {
            EfsRead::Error("efs read output has no declared data length".to_string())
        }
    } else if ril.len() >= n {
        EfsRead::Present(ril[..n].to_vec())
    } else if mtb.len() >= n {
        EfsRead::Present(mtb[..n].to_vec())
    } else {
        EfsRead::Error(format!(
            "efs read truncated: declared {} bytes, got mtb={} ril={}",
            n,
            mtb.len(),
            ril.len()
        ))
    }
}

pub fn validate_backup_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("invalid backup id".to_string());
    }
    if id == "latest" {
        return Ok(());
    }
    if id.contains("..")
        || id.starts_with('.')
        || id.starts_with('-')
        || id.contains('/')
        || id.contains('\\')
    {
        return Err("invalid backup id".to_string());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err("invalid backup id".to_string());
    }
    Ok(())
}

pub fn parse_diag_response(raw: &str) -> Vec<u8> {
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("rsp data:") {
            let data_part = &trimmed["rsp data:".len()..];
            return data_part
                .split_whitespace()
                .filter_map(|tok| {
                    if tok.starts_with("0x") || tok.starts_with("0X") {
                        u8::from_str_radix(&tok[2..], 16).ok()
                    } else {
                        None
                    }
                })
                .collect();
        }
    }
    Vec::new()
}

/// Read an Android system property. Returns "unknown" when unavailable
/// (e.g. host environments without getprop).
pub fn getprop(name: &str) -> String {
    match std::process::Command::new("getprop").arg(name).output() {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() { "unknown".to_string() } else { s }
        }
        Err(_) => "unknown".to_string(),
    }
}
