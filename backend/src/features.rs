use serde_json::{json, Value};

use crate::backup::{create_backup, list_backups, Backup, BackupEntry};
use crate::mtb::{exec_mtb, exec_mtb_owned, FileLock};
use crate::util::{
    bytes_to_hex, parse_efs_read_output, parse_hex, parse_space_dec, validate_slot, EfsRead,
};

pub struct NvWriteDef {
    pub path: &'static str,
    pub bytes: &'static str,
}

pub struct FeatureDef {
    pub id: &'static str,
    pub label: &'static str,
    pub reads: &'static [&'static str],
    pub writes: &'static [NvWriteDef],
    pub is_disabled_fn: fn(&[&[u8]]) -> bool,
}

pub const ALL_FEATURES: &[FeatureDef] = &[
    FeatureDef {
        id: "r17_2t2t",
        label: "Disable R17 2T2T UL Tx Switching",
        reads: &["/nv/item_files/modem/nr5g/RRC/cap_control_nrca_xf_plus_yt_swul_r17_band_combos_v2"],
        writes: &[NvWriteDef {
            path: "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_xf_plus_yt_swul_r17_band_combos_v2",
            bytes: "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0",
        }],
        is_disabled_fn: |byte_arrays| {
            let b = match byte_arrays.first() {
                Some(b) => b,
                None => return true,
            };
            b.iter().all(|&x| x == 0)
        },
    },
    FeatureDef {
        id: "r16_2t1t",
        label: "Disable R16 2T1T UL Tx Switching",
        reads: &[
            "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_xf_plus_yt_swul_band_combos_v2",
            "/nv/item_files/modem/nr5g/RRC/cap_swul_type_control",
        ],
        writes: &[
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_xf_plus_yt_swul_band_combos_v2",
                bytes: "0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0",
            },
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_swul_type_control",
                bytes: "0 0 0",
            },
        ],
        is_disabled_fn: |byte_arrays| {
            if byte_arrays.len() < 2 {
                return false;
            }
            let (nrca, swul) = (byte_arrays[0], byte_arrays[1]);
            nrca.iter().all(|&x| x == 0) && swul.iter().all(|&x| x == 0)
        },
    },
    FeatureDef {
        id: "ul_mimo",
        label: "Disable UL MIMO",
        reads: &["/nv/item_files/modem/nr5g/RRC/cap_limit_rf_mimo"],
        writes: &[NvWriteDef {
            path: "/nv/item_files/modem/nr5g/RRC/cap_limit_rf_mimo",
            bytes: "1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0",
        }],
        is_disabled_fn: |byte_arrays| {
            let b = match byte_arrays.first() {
                Some(b) => b,
                None => return true,
            };
            b.len() >= 3 && b[0] == 1 && b[1] == 1 && b[2] == 0
        },
    },
    FeatureDef {
        id: "fdd_ul_mimo",
        label: "Disable FDD-only UL MIMO",
        reads: &["/nv/item_files/modem/nr5g/RRC/cap_control_fdd_ul_mimo"],
        writes: &[NvWriteDef {
            path: "/nv/item_files/modem/nr5g/RRC/cap_control_fdd_ul_mimo",
            bytes: "0 0",
        }],
        is_disabled_fn: |byte_arrays| {
            let b = match byte_arrays.first() {
                Some(b) => b,
                None => return true,
            };
            b.len() >= 2 && b[0] == 0 && b[1] == 0
        },
    },
    FeatureDef {
        id: "nr_ulca",
        label: "Disable NR UL-CA",
        reads: &[
            "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_2x_f_plus_t_band_combos",
            "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_3x_f_plus_t_band_combos",
            "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_4x_f_plus_t_band_combos",
            "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_4x_f_plus_t_band_combos_v2",
            "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_f_plus_f_ulca_band_combos",
        ],
        writes: &[
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_2x_f_plus_t_band_combos",
                bytes: "0",
            },
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_3x_f_plus_t_band_combos",
                bytes: "1 1 1 1 0 0",
            },
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_4x_f_plus_t_band_combos",
                bytes: "0 1 1",
            },
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_4x_f_plus_t_band_combos_v2",
                bytes: "0 1 1 0 1 1",
            },
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_nrca_f_plus_f_ulca_band_combos",
                bytes: "0 0",
            },
        ],
        is_disabled_fn: |byte_arrays| {
            if byte_arrays.len() < 5 {
                return false;
            }
            let (b0, b1, b2, b3, b4) = (
                byte_arrays[0],
                byte_arrays[1],
                byte_arrays[2],
                byte_arrays[3],
                byte_arrays[4],
            );
            b0.first() == Some(&0)
                && b1.len() >= 6
                && b1[..6] == [1, 1, 1, 1, 0, 0]
                && b2.len() >= 3
                && b2[..3] == [0, 1, 1]
                && b3.len() >= 6
                && b3[..6] == [0, 1, 1, 0, 1, 1]
                && b4.len() >= 2
                && b4[..2] == [0, 0]
        },
    },
    FeatureDef {
        id: "dl_nrca",
        label: "Disable NR DL-CA",
        reads: &["/nv/item_files/modem/nr5g/RRC/cap_nrca_downgrade_1cc"],
        writes: &[NvWriteDef {
            path: "/nv/item_files/modem/nr5g/RRC/cap_nrca_downgrade_1cc",
            bytes: "1",
        }],
        is_disabled_fn: |byte_arrays| {
            let b = match byte_arrays.first() {
                Some(b) => b,
                None => return true,
            };
            b.first() == Some(&1)
        },
    },
    FeatureDef {
        id: "lowband_4rx",
        label: "Disable Lowbands 4Rx",
        reads: &["/nv/item_files/modem/nr5g/RRC/cap_limit_rf_mimo"],
        writes: &[NvWriteDef {
            path: "/nv/item_files/modem/nr5g/RRC/cap_limit_rf_mimo",
            bytes: "0 0 0 5 8 0 0 2 20 0 0 2 26 0 0 2 28 0 0 2 71 0 0 2 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0",
        }],
        is_disabled_fn: |byte_arrays| {
            let b = match byte_arrays.first() {
                Some(b) => b,
                None => return true,
            };
            let band_bytes = [
                8, 0, 0, 2, 20, 0, 0, 2, 26, 0, 0, 2, 28, 0, 0, 2, 71, 0, 0, 2,
            ];
            b.len() >= 24 && b[..4] == [0, 0, 0, 5] && b[4..24] == band_bytes
        },
    },
    FeatureDef {
        id: "nsa_tf_nrca",
        label: "Disable T+F NSA NR-CA",
        reads: &[
            "/nv/item_files/modem/nr5g/RRC/cap_control_mrdc_f_plus_t_band_combos",
            "/nv/item_files/modem/nr5g/RRC/cap_control_t_plus_f_band_combos",
        ],
        writes: &[
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_mrdc_f_plus_t_band_combos",
                bytes: "0",
            },
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_t_plus_f_band_combos",
                bytes: "7",
            },
        ],
        is_disabled_fn: |byte_arrays| {
            if byte_arrays.len() < 2 {
                return false;
            }
            let (b0, b1) = (byte_arrays[0], byte_arrays[1]);
            b0.first() == Some(&0) && b1.first() == Some(&7)
        },
    },
    FeatureDef {
        id: "nsa_ff_nrca",
        label: "Disable F+F NSA NR-CA",
        reads: &["/nv/item_files/modem/nr5g/RRC/cap_control_mrdc_2x_f_plus_f_band_combos"],
        writes: &[NvWriteDef {
            path: "/nv/item_files/modem/nr5g/RRC/cap_control_mrdc_2x_f_plus_f_band_combos",
            bytes: "0",
        }],
        is_disabled_fn: |byte_arrays| {
            let b = match byte_arrays.first() {
                Some(b) => b,
                None => return true,
            };
            b.first() == Some(&0)
        },
    },
    FeatureDef {
        id: "nsa_tt_nrca",
        label: "Disable T+T NSA NR-CA",
        reads: &[
            "/nv/item_files/modem/nr5g/RRC/cap_control_mrdc_t_plus_t_band_combos",
            "/nv/item_files/modem/nr5g/RRC/cap_control_nr_t_plus_t_band_combos",
        ],
        writes: &[
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_mrdc_t_plus_t_band_combos",
                bytes: "0 0 0",
            },
            NvWriteDef {
                path: "/nv/item_files/modem/nr5g/RRC/cap_control_nr_t_plus_t_band_combos",
                bytes: "0 0",
            },
        ],
        is_disabled_fn: |byte_arrays| {
            if byte_arrays.len() < 2 {
                return false;
            }
            let (b0, b1) = (byte_arrays[0], byte_arrays[1]);
            b0.len() >= 3 && b0[..3] == [0, 0, 0] && b1.len() >= 2 && b1[..2] == [0, 0]
        },
    },
    FeatureDef {
        id: "segmentation",
        label: "Disable Segmentation",
        reads: &["/nv/item_files/modem/nr5g/RRC/cap_msg_segmentation"],
        writes: &[NvWriteDef {
            path: "/nv/item_files/modem/nr5g/RRC/cap_msg_segmentation",
            bytes: "0",
        }],
        is_disabled_fn: |byte_arrays| {
            let b = match byte_arrays.first() {
                Some(b) => b,
                None => return true,
            };
            b.first() == Some(&0)
        },
    },
    FeatureDef {
        id: "dss",
        label: "Disable DSS",
        reads: &["/nv/item_files/modem/nr5g/RRC/cap_dss_control"],
        writes: &[NvWriteDef {
            path: "/nv/item_files/modem/nr5g/RRC/cap_dss_control",
            bytes: "0 0",
        }],
        is_disabled_fn: |byte_arrays| {
            let b = match byte_arrays.first() {
                Some(b) => b,
                None => return true,
            };
            b.len() >= 2 && b[..2] == [0, 0]
        },
    },
];

/// Classify a feature from the EfsRead state of every required path.
/// Any Error => "error". A true Absent (no data) => "absent" (modem default,
/// never cleared). is_disabled is only evaluated when EVERY read is Present.
pub fn classify_feature_status(
    states: &[EfsRead],
    is_disabled: &dyn Fn(&[&[u8]]) -> bool,
) -> &'static str {
    if states.iter().any(|s| matches!(s, EfsRead::Error(_))) {
        return "error";
    }
    if !states.iter().all(|s| matches!(s, EfsRead::Present(_))) {
        return "absent";
    }
    let existing: Vec<&[u8]> = states
        .iter()
        .filter_map(|s| match s {
            EfsRead::Present(b) => Some(b.as_slice()),
            _ => None,
        })
        .collect();
    if is_disabled(&existing) {
        "disabled"
    } else {
        "enabled"
    }
}

pub fn check_features(slot: i32) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let mut list = Vec::new();
    let mut failed_paths: Vec<Value> = Vec::new();
    let mut any_error = false;

    for feat in ALL_FEATURES {
        let mut path_objs = Vec::new();
        let mut states: Vec<EfsRead> = Vec::new();

        for path in feat.reads.iter() {
            let (exit, raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
            let st = parse_efs_read_output(exit, &raw);
            match &st {
                EfsRead::Present(bytes) => {
                    path_objs.push(json!({
                        "path": path,
                        "absent": false,
                        "bytes": bytes_to_hex(bytes)
                    }));
                }
                EfsRead::Absent => {
                    path_objs.push(json!({
                        "path": path,
                        "absent": true,
                        "bytes": ""
                    }));
                }
                EfsRead::Error(e) => {
                    any_error = true;
                    failed_paths.push(json!({ "path": path, "error": e }));
                    path_objs.push(json!({
                        "path": path,
                        "absent": Value::Null,
                        "bytes": "",
                        "error": e
                    }));
                }
            }
            states.push(st);
        }

        let status = classify_feature_status(&states, &feat.is_disabled_fn);

        list.push(json!({
            "id": feat.id,
            "label": feat.label,
            "status": status,
            "paths": path_objs
        }));
    }

    json!({
        "ok": !any_error,
        "features": list,
        "failed_paths": failed_paths
    })
}

pub fn disable_feature(id: &str, slot: i32) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let feat = match ALL_FEATURES.iter().find(|f| f.id == id) {
        Some(f) => f,
        None => return json!({ "ok": false, "error": format!("Unknown feature id: {}", id) }),
    };

    let _lock = match FileLock::acquire() {
        Ok(l) => l,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    // 1. Read current state of write paths for backup; if EfsRead::Error -> abort
    let mut before_states: Vec<(String, Option<Vec<u8>>)> = Vec::new();
    let mut backup_entries = Vec::new();

    for w in feat.writes {
        let (exit, raw) = exec_mtb(&["4", "4", &slot.to_string(), w.path]);
        match parse_efs_read_output(exit, &raw) {
            EfsRead::Present(b) => {
                before_states.push((w.path.to_string(), Some(b.clone())));
                backup_entries.push(BackupEntry::new(slot, w.path.to_string(), Some(bytes_to_hex(&b))));
            }
            EfsRead::Absent => {
                before_states.push((w.path.to_string(), None));
                backup_entries.push(BackupEntry::new(slot, w.path.to_string(), None));
            }
            EfsRead::Error(e) => {
                return json!({ "ok": false, "error": format!("Read failed for {}: {}", w.path, e) });
            }
        }
    }

    // 2. Create backup
    let reason = format!("feature_disable_{}", id);
    let backup = match create_backup(&reason, backup_entries) {
        Ok(b) => b,
        Err(e) => {
            return json!({
                "ok": false,
                "error": format!("Backup failed, disable aborted: {}", e)
            });
        }
    };

    // 3. Write each path (one decimal byte per argv)
    let mut writes = Vec::new();
    let mut write_failed = false;

    for w in feat.writes {
        let raw_bytes = match parse_space_dec(&w.bytes) {
            Ok(b) => b,
            Err(e) => {
                let rollback_obj = crate::util::perform_verified_rollback(slot, &before_states);
                return json!({
                    "ok": false,
                    "error": format!("Bad feature payload for {}: {}", w.path, e),
                    "rollback": rollback_obj
                });
            }
        };
        let mut write_args: Vec<String> =
            vec!["4".into(), "5".into(), slot.to_string(), w.path.to_string()];
        write_args.extend(raw_bytes.iter().map(|b| b.to_string()));
        let (write_exit, _) = exec_mtb_owned(write_args);
        if write_exit != 0 {
            write_failed = true;
        }
    }

    // 4. Re-read each & verify
    let mut reread_failed = false;
    for w in feat.writes {
        let raw_bytes = parse_space_dec(&w.bytes).unwrap_or_default();
        let expected = bytes_to_hex(&raw_bytes);
        let (r_exit, r_raw) = exec_mtb(&["4", "4", &slot.to_string(), w.path]);
        let r_read = parse_efs_read_output(r_exit, &r_raw);
        let verified = match r_read {
            EfsRead::Present(r_bytes) => bytes_to_hex(&r_bytes) == expected,
            _ => false,
        };
        if !verified {
            reread_failed = true;
        }
        writes.push(json!({
            "path": w.path,
            "backup_id": backup.id,
            "expected": expected,
            "verified": verified
        }));
    }

    if write_failed || reread_failed {
        let rollback_obj = crate::util::perform_verified_rollback(slot, &before_states);
        return json!({
            "ok": false,
            "error": "feature disable failed, rolled back",
            "id": id,
            "writes": writes,
            "rollback": rollback_obj
        });
    }

    json!({
        "ok": true,
        "id": id,
        "writes": writes
    })
}

pub fn restore_feature(id: &str, slot: i32) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let feat = match ALL_FEATURES.iter().find(|f| f.id == id) {
        Some(f) => f,
        None => return json!({ "ok": false, "error": format!("Unknown feature id: {}", id) }),
    };

    let _lock = match FileLock::acquire() {
        Ok(l) => l,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    // Look for backup & pre-read before-state of target paths
    let backups_list = list_backups().unwrap_or_default();
    let mut before_states: Vec<(String, Option<Vec<u8>>)> = Vec::new();
    let mut target_entries = Vec::new();

    for w in feat.writes {
        // Read current state before restore
        let (exit, raw) = exec_mtb(&["4", "4", &slot.to_string(), w.path]);
        match parse_efs_read_output(exit, &raw) {
            EfsRead::Present(b) => before_states.push((w.path.to_string(), Some(b))),
            EfsRead::Absent => before_states.push((w.path.to_string(), None)),
            EfsRead::Error(e) => {
                return json!({ "ok": false, "error": format!("Read failed for {}: {}", w.path, e) });
            }
        }

    }

    // Restore MUST come from ONE transaction manifest: the latest backup with
    // reason "feature_disable_<id>" that contains EVERY write path of this
    // feature for the target slot. Mixing entries from different transactions
    // (features share NV paths, e.g. MIMO) could silently restore a wrong
    // state.
    let expected_reason = format!("feature_disable_{}", id);
    let mut manifest: Option<Backup> = None;
    for b_val in &backups_list {
        if let Ok(b) = serde_json::from_value::<Backup>(b_val.clone()) {
            if b.reason != expected_reason {
                continue;
            }
            let has_all = feat.writes.iter().all(|w| {
                b.entries.iter().any(|e| e.slot == slot && e.path == w.path)
            });
            if has_all {
                manifest = Some(b);
                break; // list is newest-first
            }
        }
    }
    let manifest = match manifest {
        Some(m) => m,
        None => {
            return json!({
                "ok": false,
                "error": format!("no complete feature_disable_{} backup found, refusing restore", id)
            });
        }
    };

    for w in feat.writes {
        let entry = match manifest.entries.iter().find(|e| e.slot == slot && e.path == w.path) {
            Some(e) => e.clone(),
            None => {
                return json!({
                    "ok": false,
                    "error": format!("manifest missing entry for {}, refusing delete-restore", w.path)
                });
            }
        };
        if let Err(err) = entry.verify_integrity() {
            return json!({ "ok": false, "error": format!("Integrity check failed: {}", err) });
        }
        target_entries.push((w.path, entry));
    }

    let mut restored = Vec::new();
    let mut write_failed = false;
    let mut reread_failed = false;

    for (path, entry) in target_entries {
        let (op, ok, verified) = match &entry.bytes {
            Some(hex_str) => {
                if let Ok(raw_bytes) = parse_hex(hex_str) {
                    let mut write_args: Vec<String> =
                        vec!["4".into(), "5".into(), slot.to_string(), path.to_string()];
                    write_args.extend(raw_bytes.iter().map(|b| b.to_string()));
                    let (exit, _) = exec_mtb_owned(write_args);
                    if exit == 0 {
                        let (r_exit, r_raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
                        let r_read = parse_efs_read_output(r_exit, &r_raw);
                        let v = match r_read {
                            EfsRead::Present(b) => bytes_to_hex(&b) == *hex_str,
                            _ => false,
                        };
                        ("write", exit == 0 && v, v)
                    } else {
                        ("write", false, false)
                    }
                } else {
                    ("write", false, false)
                }
            }
            None => {
                let (exit, _) = exec_mtb(&["4", "6", &slot.to_string(), path]);
                if exit == 0 {
                    let (r_exit, r_raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
                    let r_read = parse_efs_read_output(r_exit, &r_raw);
                    let v = matches!(r_read, EfsRead::Absent);
                    ("delete", exit == 0 && v, v)
                } else {
                    ("delete", false, false)
                }
            }
        };

        if !ok { write_failed = true; }
        if !verified { reread_failed = true; }

        restored.push(json!({
            "path": path,
            "op": op,
            "ok": ok,
            "verified": verified
        }));
    }

    if write_failed || reread_failed {
        let rollback_obj = crate::util::perform_verified_rollback(slot, &before_states);
        return json!({
            "ok": false,
            "error": "feature restore failed, rolled back",
            "id": id,
            "restored": restored,
            "rollback": rollback_obj
        });
    }

    json!({
        "ok": true,
        "id": id,
        "restored": restored
    })
}
