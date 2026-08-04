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

pub fn parse_efs_read_output(exit_code: i32, raw: &str) -> EfsRead {
    if exit_code != 0 {
        return EfsRead::Error(format!("exit {}", exit_code));
    }

    let mut bytes = Vec::new();
    for line in raw.lines() {
        if line.contains("xiaomi_nvefs_test_efs_read:") {
            if let Some(last) = line.trim().split_whitespace().last() {
                if last.len() == 2 && last.chars().all(|c| c.is_ascii_hexdigit()) {
                    if let Ok(b) = u8::from_str_radix(last, 16) {
                        bytes.push(b);
                    }
                }
            }
        }
    }

    if bytes.is_empty() {
        EfsRead::Absent
    } else {
        EfsRead::Present(bytes)
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
