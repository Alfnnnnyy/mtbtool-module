use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::backup::{create_backup, BackupEntry};
use crate::mtb::{exec_mtb, exec_mtb_owned, FileLock};
use crate::util::{
    bytes_to_hex, parse_diag_response, parse_efs_read_output, validate_slot, EfsRead,
};

pub const ALL_LTE_BANDS: &[i32] = &[
    1, 2, 3, 4, 5, 7, 8, 12, 13, 14, 17, 18, 19, 20, 21, 25, 26, 28, 29, 30, 32, 34, 38, 39, 40,
    41, 42, 43, 46, 48, 66, 71,
];

pub const ALL_NR_BANDS: &[i32] = &[
    1, 2, 3, 5, 7, 8, 12, 14, 18, 20, 25, 26, 28, 29, 30, 34, 38, 39, 40, 41, 46, 48, 50, 51, 53,
    65, 66, 70, 71, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 86, 89, 90, 91, 92, 93, 94, 95, 96,
    97, 100, 101, 102, 104, 257, 258, 260, 261,
];

#[derive(Debug, Clone)]
pub struct SlotPaths {
    pub lte_primary: String,
    pub lte_extension: String,
    pub nr: String,
    pub nr_nsa: String,
}

pub fn paths_for_slot(slot: i32) -> SlotPaths {
    if slot == 1 {
        SlotPaths {
            lte_primary: "/nv/item_files/modem/mmode/lte_bandpref_Subscription01".to_string(),
            lte_extension:
                "/nv/item_files/modem/mmode/lte_bandpref_extn_65_256_Subscription01".to_string(),
            nr: "/nv/item_files/modem/mmode/nr_band_pref_Subscription01".to_string(),
            nr_nsa: "/nv/item_files/modem/mmode/nr_nsa_band_pref_Subscription01".to_string(),
        }
    } else {
        SlotPaths {
            lte_primary: "/nv/item_files/modem/mmode/lte_bandpref".to_string(),
            lte_extension: "/nv/item_files/modem/mmode/lte_bandpref_extn_65_256".to_string(),
            nr: "/nv/item_files/modem/mmode/nr_band_pref".to_string(),
            nr_nsa: "/nv/item_files/modem/mmode/nr_nsa_band_pref".to_string(),
        }
    }
}

pub fn parse_lte_primary(bytes: &[u8]) -> Vec<i32> {
    let mut res = Vec::new();
    for band in 1..=64 {
        let bit_idx = band - 1;
        let byte_idx = (bit_idx / 8) as usize;
        let bit_in_byte = bit_idx % 8;
        let b = bytes.get(byte_idx).copied().unwrap_or(0);
        if (b >> bit_in_byte) & 1 == 1 {
            res.push(band);
        }
    }
    res
}

pub fn parse_lte_extension(bytes: &[u8]) -> Vec<i32> {
    let mut res = Vec::new();
    for &band in &[66, 71] {
        let bit_idx = band - 65;
        let byte_idx = (bit_idx / 8) as usize;
        let bit_in_byte = bit_idx % 8;
        let b = bytes.get(byte_idx).copied().unwrap_or(0);
        if (b >> bit_in_byte) & 1 == 1 {
            res.push(band);
        }
    }
    res
}

pub fn parse_nr_bitmask(bytes: &[u8]) -> Vec<i32> {
    let mut res = Vec::new();
    for &band in ALL_NR_BANDS {
        let bit_idx = band - 1;
        let byte_idx = (bit_idx / 8) as usize;
        let bit_in_byte = bit_idx % 8;
        let b = bytes.get(byte_idx).copied().unwrap_or(0);
        if (b >> bit_in_byte) & 1 == 1 {
            res.push(band);
        }
    }
    res
}

pub fn build_lte_primary(enabled_bands: &[i32]) -> Vec<u8> {
    let mut bytes = vec![0u8; 8];
    for &band in enabled_bands {
        if !(1..=64).contains(&band) {
            continue;
        }
        let bit_idx = band - 1;
        let byte_idx = (bit_idx / 8) as usize;
        let bit_in_byte = bit_idx % 8;
        bytes[byte_idx] |= 1 << bit_in_byte;
    }
    bytes
}

pub fn build_lte_extension(enabled_bands: &[i32]) -> Vec<u8> {
    let mut bytes = vec![0u8; 24];
    for &band in &[66, 71] {
        if !enabled_bands.contains(&band) {
            continue;
        }
        let bit_idx = band - 65;
        let byte_idx = (bit_idx / 8) as usize;
        let bit_in_byte = bit_idx % 8;
        bytes[byte_idx] |= 1 << bit_in_byte;
    }
    bytes
}

pub fn build_nr_bitmask(enabled_bands: &[i32]) -> Vec<u8> {
    let mut bytes = vec![0u8; 64];
    for &band in enabled_bands {
        if band < 1 {
            continue;
        }
        let bit_idx = band - 1;
        let byte_idx = (bit_idx / 8) as usize;
        let bit_in_byte = bit_idx % 8;
        if byte_idx < bytes.len() {
            bytes[byte_idx] |= 1 << bit_in_byte;
        }
    }
    bytes
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BandOffsets {
    pub lte: usize,
    pub nr_sa: usize,
    pub nr_nsa: usize,
}

pub fn detect_band_offsets(diag_bytes: &[u8], min_bands: usize) -> BandOffsets {
    let nr_bands_sub80: Vec<i32> = ALL_NR_BANDS.iter().copied().filter(|&b| b <= 80).collect();

    let scan_candidates = |length: usize, known_bands: &[i32]| -> Vec<(usize, usize)> {
        let mut results = Vec::new();
        if diag_bytes.len() < length {
            return results;
        }
        let max_offset = diag_bytes.len() - length;
        for offset in 0..=max_offset {
            let mut found = 0usize;
            let mut spurious = 0usize;
            for bit in 0..(length * 8) {
                let band = (bit + 1) as i32;
                let byte_index = offset + bit / 8;
                let bit_in_byte = bit % 8;
                if (diag_bytes[byte_index] >> bit_in_byte) & 1 == 1 {
                    if known_bands.contains(&band) {
                        found += 1;
                    } else {
                        spurious += 1;
                    }
                }
            }
            if spurious == 0 && found >= min_bands {
                results.push((found, offset));
            }
        }
        results.sort_by(|a, b| b.0.cmp(&a.0));
        results
    };

    let lte_candidates = scan_candidates(9, ALL_LTE_BANDS);
    let lte_offset = lte_candidates.first().map(|&(_, off)| off).unwrap_or(36);

    let nr_candidates = scan_candidates(10, &nr_bands_sub80);
    let mut nr_offsets = Vec::new();
    for &(_, offset) in &nr_candidates {
        if nr_offsets.iter().all(|&o: &usize| (o as isize - offset as isize).abs() >= 10) {
            nr_offsets.push(offset);
        }
        if nr_offsets.len() == 2 {
            break;
        }
    }
    nr_offsets.sort();
    let nr_sa_offset = nr_offsets.get(0).copied().unwrap_or(108);
    let nr_nsa_offset = nr_offsets
        .get(1)
        .copied()
        .unwrap_or_else(|| nr_offsets.get(0).copied().unwrap_or(172));

    BandOffsets {
        lte: lte_offset,
        nr_sa: nr_sa_offset,
        nr_nsa: nr_nsa_offset,
    }
}

pub fn parse_bitmask_bands(
    diag_bytes: &[u8],
    offset: usize,
    length: usize,
    known_bands: &[i32],
) -> Vec<i32> {
    let mut result = Vec::new();
    for &band in known_bands {
        let bit_index = band - 1;
        let byte_index = offset + (bit_index / 8) as usize;
        let bit_in_byte = bit_index % 8;
        if byte_index >= offset + length {
            continue;
        }
        if byte_index < diag_bytes.len() {
            if (diag_bytes[byte_index] >> bit_in_byte) & 1 == 1 {
                result.push(band);
            }
        }
    }
    result
}

pub fn get_bandlock(slot: i32) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let paths = paths_for_slot(slot);
    let mut errors = serde_json::Map::new();
    let mut failed_paths = Vec::new();

    let read_path = |path: &str| -> (bool, Vec<u8>, Option<String>) {
        let (exit, raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
        match parse_efs_read_output(exit, &raw) {
            EfsRead::Present(bytes) => (false, bytes, None),
            EfsRead::Absent => (true, Vec::new(), None),
            EfsRead::Error(e) => (true, Vec::new(), Some(e)),
        }
    };

    let (lte_pri_absent, lte_pri_bytes, err1) = read_path(&paths.lte_primary);
    if let Some(e) = err1 {
        errors.insert(paths.lte_primary.clone(), json!(e));
        failed_paths.push(paths.lte_primary.clone());
    }

    let (lte_ext_absent, lte_ext_bytes, err2) = read_path(&paths.lte_extension);
    if let Some(e) = err2 {
        errors.insert(paths.lte_extension.clone(), json!(e));
        failed_paths.push(paths.lte_extension.clone());
    }

    let (nr_nsa_absent, nr_nsa_bytes, err3) = read_path(&paths.nr_nsa);
    if let Some(e) = err3 {
        errors.insert(paths.nr_nsa.clone(), json!(e));
        failed_paths.push(paths.nr_nsa.clone());
    }

    let (nr_sa_absent, nr_sa_bytes, err4) = read_path(&paths.nr);
    if let Some(e) = err4 {
        errors.insert(paths.nr.clone(), json!(e));
        failed_paths.push(paths.nr.clone());
    }

    let lte_pri_hex = if lte_pri_absent { "".to_string() } else { bytes_to_hex(&lte_pri_bytes) };
    let lte_ext_hex = if lte_ext_absent { "".to_string() } else { bytes_to_hex(&lte_ext_bytes) };
    let nr_nsa_hex = if nr_nsa_absent { "".to_string() } else { bytes_to_hex(&nr_nsa_bytes) };
    let nr_sa_hex = if nr_sa_absent { "".to_string() } else { bytes_to_hex(&nr_sa_bytes) };

    let mut lte_bands: BTreeSet<i32> = parse_lte_primary(&lte_pri_bytes).into_iter().collect();
    for b in parse_lte_extension(&lte_ext_bytes) {
        lte_bands.insert(b);
    }
    let nr_nsa_bands: BTreeSet<i32> = parse_nr_bitmask(&nr_nsa_bytes).into_iter().collect();
    let nr_sa_bands: BTreeSet<i32> = parse_nr_bitmask(&nr_sa_bytes).into_iter().collect();

    let is_ok = failed_paths.is_empty();
    let mut res = json!({
        "ok": is_ok,
        "paths": {
            "ltePrimary": paths.lte_primary,
            "lteExtension": paths.lte_extension,
            "nrNsa": paths.nr_nsa,
            "nr": paths.nr
        },
        "bytes": {
            "ltePrimary": lte_pri_hex,
            "lteExtension": lte_ext_hex,
            "nrNsa": nr_nsa_hex,
            "nr": nr_sa_hex
        },
        "bands": {
            "lte": lte_bands.into_iter().collect::<Vec<_>>(),
            "nrNsa": nr_nsa_bands.into_iter().collect::<Vec<_>>(),
            "nrSa": nr_sa_bands.into_iter().collect::<Vec<_>>()
        },
        "errors": errors
    });

    if !is_ok {
        res["error"] = json!(format!("read failed for {}", failed_paths.join(", ")));
    }

    res
}

pub fn parse_band_list(
    s: Option<&str>,
    known: &[i32],
    label: &str,
    allow_empty: bool,
) -> Result<Option<Vec<i32>>, String> {
    let s = match s {
        Some(v) => v,
        None => return Ok(None),
    };
    let trimmed = s.trim();
    if trimmed.is_empty() {
        if allow_empty {
            return Ok(Some(Vec::new()));
        } else {
            return Err(format!("empty band list for {} requires allowEmpty", label));
        }
    }
    let mut res = Vec::new();
    for tok in trimmed.split(|c: char| c == ',' || c.is_whitespace()) {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let val = tok
            .parse::<i32>()
            .map_err(|_| format!("invalid band token '{}' for {}", tok, label))?;
        if !known.contains(&val) {
            return Err(format!("invalid band token '{}' for {}", tok, label));
        }
        res.push(val);
    }
    Ok(Some(res))
}

pub fn set_bandlock(
    slot: i32,
    lte_str: Option<&str>,
    nr_nsa_str: Option<&str>,
    nr_sa_str: Option<&str>,
    allow_empty: bool,
) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let lte_bands = match parse_band_list(lte_str, ALL_LTE_BANDS, "lte", allow_empty) {
        Ok(b) => b,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    let nr_nsa_bands = match parse_band_list(nr_nsa_str, ALL_NR_BANDS, "nrNsa", allow_empty) {
        Ok(b) => b,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    let nr_sa_bands = match parse_band_list(nr_sa_str, ALL_NR_BANDS, "nrSa", allow_empty) {
        Ok(b) => b,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    if lte_bands.is_none() && nr_nsa_bands.is_none() && nr_sa_bands.is_none() {
        return json!({
            "ok": false,
            "error": "refusing zero-band mask: no band categories provided"
        });
    }

    let paths = paths_for_slot(slot);
    let mut targets: Vec<(String, Vec<u8>)> = Vec::new();

    if let Some(bands) = &lte_bands {
        targets.push((paths.lte_primary.clone(), build_lte_primary(bands)));
        targets.push((paths.lte_extension.clone(), build_lte_extension(bands)));
    }
    if let Some(bands) = &nr_nsa_bands {
        targets.push((paths.nr_nsa.clone(), build_nr_bitmask(bands)));
    }
    if let Some(bands) = &nr_sa_bands {
        targets.push((paths.nr.clone(), build_nr_bitmask(bands)));
    }

    let _lock = match FileLock::acquire() {
        Ok(l) => l,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    // Read touched target paths first; any Error aborts
    let mut before_states: Vec<(String, Option<Vec<u8>>, Vec<u8>)> = Vec::new();
    let mut backup_entries = Vec::new();

    for (path, new_bytes) in &targets {
        let (exit_before, raw_before) = exec_mtb(&["4", "4", &slot.to_string(), path]);
        match parse_efs_read_output(exit_before, &raw_before) {
            EfsRead::Present(b) => {
                before_states.push((path.clone(), Some(b.clone()), new_bytes.clone()));
                backup_entries.push(BackupEntry::new(slot, path.clone(), Some(bytes_to_hex(&b))));
            }
            EfsRead::Absent => {
                before_states.push((path.clone(), None, new_bytes.clone()));
                backup_entries.push(BackupEntry::new(slot, path.clone(), None));
            }
            EfsRead::Error(e) => {
                return json!({
                    "ok": false,
                    "error": e,
                    "aborted_at": path
                });
            }
        }
    }

    // Build ONE backup
    let backup = match create_backup("bandlock_set", backup_entries) {
        Ok(b) => b,
        Err(e) => {
            return json!({ "ok": false, "error": format!("Backup failed: {}", e) });
        }
    };

    // Write touched paths
    let mut write_failed = false;
    let mut writes = Vec::new();

    for (path, _before, new_bytes) in &before_states {
        let mut write_args: Vec<String> = vec!["4".into(), "5".into(), slot.to_string(), path.clone()];
        write_args.extend(new_bytes.iter().map(|b| b.to_string()));
        let (write_exit, _) = exec_mtb_owned(write_args);
        writes.push(json!({
            "path": path,
            "action": "write",
            "backup_id": backup.id
        }));
        if write_exit != 0 {
            write_failed = true;
        }
    }

    // Re-read all touched paths & verify
    let mut verified = serde_json::Map::new();
    let mut reread_failed = false;

    for (path, _before, new_bytes) in &before_states {
        let (exit_after, raw_after) = exec_mtb(&["4", "4", &slot.to_string(), path]);
        match parse_efs_read_output(exit_after, &raw_after) {
            EfsRead::Present(bytes_after) => {
                let is_match = bytes_after == *new_bytes;
                if !is_match {
                    reread_failed = true;
                }
                verified.insert(
                    path.clone(),
                    json!({
                        "bytes": bytes_to_hex(&bytes_after),
                        "match": is_match
                    }),
                );
            }
            _ => {
                reread_failed = true;
                verified.insert(
                    path.clone(),
                    json!({
                        "bytes": "",
                        "match": false
                    }),
                );
            }
        }
    }

    // If any failure -> ROLLBACK
    if write_failed || reread_failed {
        let rollback_before: Vec<(String, Option<Vec<u8>>)> = before_states
            .iter()
            .map(|(p, b, _)| (p.clone(), b.clone()))
            .collect();
        let rollback_obj = crate::util::perform_verified_rollback(slot, &rollback_before);
        return json!({
            "ok": false,
            "error": "band lock failed, rolled back",
            "writes": writes,
            "verified": verified,
            "rollback": rollback_obj
        });
    }

    // Success response
    json!({
        "ok": true,
        "writes": writes,
        "verified": verified,
        "backup": backup.id
    })
}
pub fn detect_bandlock(slot: i32) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let (exit, raw) = exec_mtb(&["5", "0", "0", "0", "1000", "75", "19", "4", "0", "0", "0", "0"]);
    if exit != 0 {
        return json!({ "ok": false, "error": "DIAG request failed" });
    }

    let bytes = parse_diag_response(&raw);
    if bytes.is_empty() {
        return json!({ "ok": false, "error": "Empty DIAG response" });
    }

    let offsets = detect_band_offsets(&bytes, 3);
    let lte_bands = parse_bitmask_bands(&bytes, offsets.lte, 9, ALL_LTE_BANDS);
    let nr_nsa_bands = parse_bitmask_bands(&bytes, offsets.nr_nsa, 16, ALL_NR_BANDS);
    let nr_sa_bands = parse_bitmask_bands(&bytes, offsets.nr_sa, 16, ALL_NR_BANDS);

    json!({
        "ok": true,
        "bands": {
            "lte": lte_bands,
            "nrNsa": nr_nsa_bands,
            "nrSa": nr_sa_bands
        },
        "offsets": offsets,
        "raw_byte_count": bytes.len()
    })
}

