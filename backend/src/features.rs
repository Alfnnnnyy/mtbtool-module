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

pub fn check_features(slot: i32) -> Value {
    if let Err(e) = validate_slot(slot) {
        return json!({ "ok": false, "error": e });
    }

    let mut list = Vec::new();

    for feat in ALL_FEATURES {
        let mut path_objs = Vec::new();
        let mut byte_arrays: Vec<Option<Vec<u8>>> = Vec::new();

        for &path in feat.reads {
            let (exit, raw) = exec_mtb(&["4", "4", &slot.to_string(), path]);
            match parse_efs_read_output(exit, &raw) {
                EfsRead::Present(bytes) => {
                    path_objs.push(json!({
                        "path": path,
                        "absent": false,
                        "bytes": bytes_to_hex(&bytes)
                    }));
                    byte_arrays.push(Some(bytes));
                }
                EfsRead::Absent => {
                    path_objs.push(json!({
                        "path": path,
                        "absent": true,
                        "bytes": ""
                    }));
                    byte_arrays.push(None);
                }
                EfsRead::Error(e) => {
                    path_objs.push(json!({
                        "path": path,
                        "absent": false,
                        "bytes": "",
                        "error": e
                    }));
                    byte_arrays.push(None);
                }
            }
        }

        let status = if byte_arrays.iter().any(|b| b.is_none()) {
            "absent"
        } else {
            let existing: Vec<&[u8]> = byte_arrays
                .iter()
                .filter_map(|b| b.as_deref())
                .collect();
            if (feat.is_disabled_fn)(&existing) {
                "disabled"
            } else {
                "enabled"
            }
        };

        list.push(json!({
            "id": feat.id,
            "label": feat.label,
            "status": status,
            "paths": path_objs
        }));
    }

    json!({
        "ok": true,
        "features": list
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

    // 1. Read current state of write paths for backup
    let mut backup_entries = Vec::new();
    for w in feat.writes {
        let (exit, raw) = exec_mtb(&["4", "4", &slot.to_string(), w.path]);
        let before_hex = match parse_efs_read_output(exit, &raw) {
            EfsRead::Present(b) => Some(bytes_to_hex(&b)),
            EfsRead::Absent => None,
            EfsRead::Error(e) => {
                return json!({ "ok": false, "error": format!("Read failed for {}: {}", w.path, e) });
            }
        };
        backup_entries.push(BackupEntry::new(slot, w.path.to_string(), before_hex));
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

    // 3. Write each path (one decimal byte per argv) + read-back verify
    let mut writes = Vec::new();
    for w in feat.writes {
        let raw_bytes = match parse_space_dec(&w.bytes) {
            Ok(b) => b,
            Err(e) => {
                return json!({ "ok": false, "error": format!("Bad feature payload for {}: {}", w.path, e) });
            }
        };
        let mut write_args: Vec<String> =
            vec!["4".into(), "5".into(), slot.to_string(), w.path.to_string()];
        write_args.extend(raw_bytes.iter().map(|b| b.to_string()));
        let (write_exit, _) = exec_mtb_owned(write_args);
        if write_exit != 0 {
            return json!({
                "ok": false,
                "error": format!("Failed to write NV path {}", w.path)
            });
        }
        let expected = bytes_to_hex(&raw_bytes);
        let (r_exit, r_raw) = exec_mtb(&["4", "4", &slot.to_string(), w.path]);
        let r_read = parse_efs_read_output(r_exit, &r_raw);
        let verified = match r_read {
            EfsRead::Present(r_bytes) => bytes_to_hex(&r_bytes) == expected,
            _ => false,
        };
        writes.push(json!({
            "path": w.path,
            "backup_id": backup.id,
            "expected": expected,
            "verified": verified
        }));
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

    // Look for backup
    let backups_list = list_backups().unwrap_or_default();
    let mut restored = Vec::new();

    for w in feat.writes {
        // Find latest backup containing entry for this slot & path
        let mut target_entry: Option<BackupEntry> = None;
        for b_val in &backups_list {
            if let Ok(b) = serde_json::from_value::<Backup>(b_val.clone()) {
                if let Some(entry) = b.entries.iter().find(|e| e.slot == slot && e.path == w.path) {
                    target_entry = Some(entry.clone());
                    break;
                }
            }
        }

        let entry = match target_entry {
            Some(e) => e,
            None => {
                return json!({
                    "ok": false,
                    "error": format!("no backup entry for {}, refusing delete-restore", w.path)
                });
            }
        };

        if let Err(err) = entry.verify_integrity() {
            return json!({ "ok": false, "error": format!("Integrity check failed: {}", err) });
        }

        let (op, ok, verified) = match &entry.bytes {
            Some(hex_str) => {
                if let Ok(raw_bytes) = parse_hex(hex_str) {
                    let mut write_args: Vec<String> =
                        vec!["4".into(), "5".into(), slot.to_string(), w.path.to_string()];
                    write_args.extend(raw_bytes.iter().map(|b| b.to_string()));
                    let (exit, _) = exec_mtb_owned(write_args);
                    if exit == 0 {
                        let (r_exit, r_raw) = exec_mtb(&["4", "4", &slot.to_string(), w.path]);
                        let r_read = parse_efs_read_output(r_exit, &r_raw);
                        let v = match r_read {
                            EfsRead::Present(b) => bytes_to_hex(&b) == *hex_str,
                            _ => false,
                        };
                        ("write", true, v)
                    } else {
                        ("write", false, false)
                    }
                } else {
                    ("write", false, false)
                }
            }
            None => {
                let (exit, _) = exec_mtb(&["4", "6", &slot.to_string(), w.path]);
                if exit == 0 {
                    let (r_exit, r_raw) = exec_mtb(&["4", "4", &slot.to_string(), w.path]);
                    let r_read = parse_efs_read_output(r_exit, &r_raw);
                    let v = matches!(r_read, EfsRead::Absent);
                    ("delete", true, v)
                } else {
                    ("delete", false, false)
                }
            }
        };

        restored.push(json!({
            "path": w.path,
            "op": op,
            "ok": ok,
            "verified": verified
        }));
    }
    json!({
        "ok": true,
        "id": id,
        "restored": restored
    })
}
