use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::backup::{create_backup, BackupEntry};
use crate::mtb::{exec_mtb_owned, exec_mtb, FileLock};
use crate::util::{
    bytes_to_hex, parse_diag_response, parse_efs_read_output,
    validate_slot,
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

    // Read 4 paths
    let read_path = |path: &str| -> (i32, bool, Vec<u8>) {
        let (exit, raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
        let (absent, bytes) = parse_efs_read_output(exit, &raw);
        (exit, absent, bytes)
    };

    let (_, lte_pri_absent, lte_pri_bytes) = read_path(&paths.lte_primary);
    let (_, lte_ext_absent, lte_ext_bytes) = read_path(&paths.lte_extension);
    let (_, nr_nsa_absent, nr_nsa_bytes) = read_path(&paths.nr_nsa);
    let (_, nr_sa_absent, nr_sa_bytes) = read_path(&paths.nr);

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

    json!({
        "ok": true,
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
        "errors": {}
    })
}

pub fn parse_band_list(s: &str) -> Vec<i32> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|tok| tok.trim().parse::<i32>().ok())
        .collect()
}

pub fn set_bandlock(
    slot: i32,
    lte_str: Option<&str>,
    nr_nsa_str: Option<&str>,
    nr_sa_str: Option<&str>,
) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let paths = paths_for_slot(slot);

    let _lock = match FileLock::acquire() {
        Ok(l) => l,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    let mut to_write: Vec<(String, Vec<u8>)> = Vec::new();

    if let Some(lte) = lte_str {
        let bands = parse_band_list(lte);
        to_write.push((paths.lte_primary.clone(), build_lte_primary(&bands)));
        to_write.push((paths.lte_extension.clone(), build_lte_extension(&bands)));
    }
    if let Some(nr_nsa) = nr_nsa_str {
        let bands = parse_band_list(nr_nsa);
        to_write.push((paths.nr_nsa.clone(), build_nr_bitmask(&bands)));
    }
    if let Some(nr_sa) = nr_sa_str {
        let bands = parse_band_list(nr_sa);
        to_write.push((paths.nr.clone(), build_nr_bitmask(&bands)));
    }

    if to_write.is_empty() {
        return json!({ "ok": false, "error": "No band parameters provided" });
    }

    let mut writes = Vec::new();
    let mut verified = serde_json::Map::new();

    for (path, new_bytes) in to_write {
        // Read before for backup
        let (exit_before, raw_before) = exec_mtb(&["4", "4", &slot.to_string(), &path]);
        let (absent_before, bytes_before) = parse_efs_read_output(exit_before, &raw_before);
        let before_hex = if absent_before { None } else { Some(bytes_to_hex(&bytes_before)) };

        let backup_entry = BackupEntry {
            slot,
            path: path.clone(),
            bytes: before_hex,
        };
        let backup = match create_backup("bandlock_set", vec![backup_entry]) {
            Ok(b) => b,
            Err(e) => {
                return json!({ "ok": false, "error": format!("Backup failed for {}: {}", path, e) });
            }
        };

        // Write new bytes — each decimal byte as a separate argv entry
        let mut write_args: Vec<String> = vec!["4".into(), "5".into(), slot.to_string(), path.to_string()];
        write_args.extend(new_bytes.iter().map(|b| b.to_string()));
        let (write_exit, _) = exec_mtb_owned(write_args);
        if write_exit != 0 {
            return json!({ "ok": false, "error": format!("Failed to write NV path {}", path) });
        }

        writes.push(json!({ "path": path, "backup_id": backup.id }));

        // Verify re-read
        let (exit_after, raw_after) = exec_mtb(&["4", "4", &slot.to_string(), &path]);
        let (_, bytes_after) = parse_efs_read_output(exit_after, &raw_after);
        verified.insert(path, json!(bytes_to_hex(&bytes_after)));
    }

    json!({
        "ok": true,
        "writes": writes,
        "verified": verified
    })
}

pub fn detect_bandlock(slot: i32) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let diag_open_args = &[
        "5", "0", "0", "0", "1000", "75", "19", "2", "0", "0", "0", "0", "0", "0", "0", "0", "0",
        "47", "112", "111", "108", "105", "99", "121", "109", "97", "47", "112", "101", "114",
        "115", "105", "115", "116", "101", "100", "95", "105", "116", "101", "109", "115", "47",
        "108", "105", "109", "105", "116", "101", "100", "95", "98", "97", "110", "100", "115",
        "0",
    ];
    let diag_read_args = &[
        "5", "0", "0", "0", "1000", "75", "19", "4", "0", "0", "0", "0", "0", "0", "1", "0", "0",
        "0", "0", "0", "0",
    ];

    let _open_res = exec_mtb(diag_open_args);
    let (_read_exit, read_raw) = exec_mtb(diag_read_args);

    let diag_bytes = parse_diag_response(&read_raw);
    let raw_byte_count = diag_bytes.len();

    let offsets = detect_band_offsets(&diag_bytes, 5);

    let lte = parse_bitmask_bands(&diag_bytes, offsets.lte, 9, ALL_LTE_BANDS);
    let nr_sa = parse_bitmask_bands(&diag_bytes, offsets.nr_sa, 10, ALL_NR_BANDS);
    let nr_nsa = parse_bitmask_bands(&diag_bytes, offsets.nr_nsa, 10, ALL_NR_BANDS);

    json!({
        "ok": true,
        "lte": lte,
        "nrNsa": nr_nsa,
        "nrSa": nr_sa,
        "offsets": {
            "lte": offsets.lte,
            "nrSa": offsets.nr_sa,
            "nrNsa": offsets.nr_nsa
        },
        "raw_byte_count": raw_byte_count
    })
}
