use mtbctl::backup::{create_backup, get_backup, list_backups, BackupEntry};
use mtbctl::bandlock::{
    build_lte_extension, build_lte_primary, build_nr_bitmask, detect_band_offsets,
    parse_bitmask_bands, parse_lte_extension, parse_lte_primary, parse_nr_bitmask, BandOffsets,
    ALL_LTE_BANDS, ALL_NR_BANDS,
};
use mtbctl::cells::{parse_asdiv_line, parse_lte_cell, parse_nr_cell, parse_tx_power, LteCellData, NrCellData};
use mtbctl::features::ALL_FEATURES;
use mtbctl::importer::parse_import_json;
use mtbctl::util::{parse_hex, validate_hex, validate_nv_path, validate_slot};
use std::env;
use std::fs;

#[test]
fn test_hex_parse_validation() {
    let valid_hex = "000102030405060708090a0b0c0d0e0f";
    assert!(validate_hex(valid_hex).is_ok());
    let parsed = parse_hex(valid_hex).unwrap();
    assert_eq!(
        parsed,
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
    );

    // Odd length
    assert!(validate_hex("0").is_err());
    // Non-hex
    assert!(validate_hex("00GG").is_err());
    // Exceeds 1024 chars
    let long_hex = "00".repeat(513); // 1026 chars
    assert!(validate_hex(&long_hex).is_err());
}

#[test]
fn test_nv_path_and_slot_validation() {
    assert!(validate_slot(0).is_ok());
    assert!(validate_slot(1).is_ok());
    assert!(validate_slot(2).is_err());

    assert!(validate_nv_path("/nv/item_files/modem/mmode/lte_bandpref").is_ok());
    assert!(validate_nv_path("/nv/item_files/modem/nr5g/RRC/cap_dss_control").is_ok());

    // Path traversal / invalid prefix
    assert!(validate_nv_path("/nv/item_files/modem/mmode/../etc/passwd").is_err());
    assert!(validate_nv_path("/etc/passwd").is_err());
}

#[test]
fn test_lte_primary_ext_nr_mask_roundtrip() {
    // LTE Primary (1..64)
    let lte_pri_enabled = vec![1, 3, 7, 28, 48];
    let lte_pri_bytes = build_lte_primary(&lte_pri_enabled);
    let lte_pri_parsed = parse_lte_primary(&lte_pri_bytes);
    assert_eq!(lte_pri_parsed, lte_pri_enabled);

    // LTE Extension (66, 71)
    let lte_ext_enabled = vec![66, 71];
    let lte_ext_bytes = build_lte_extension(&lte_ext_enabled);
    let lte_ext_parsed = parse_lte_extension(&lte_ext_bytes);
    assert_eq!(lte_ext_parsed, lte_ext_enabled);

    // NR Bitmask (ALL_NR_BANDS)
    let nr_enabled = vec![1, 78, 257];
    let nr_bytes = build_nr_bitmask(&nr_enabled);
    let nr_parsed = parse_nr_bitmask(&nr_bytes);
    assert_eq!(nr_parsed, nr_enabled);
}

#[test]
fn test_diag_offset_detection() {
    // 1) Default layout offsets: lte=36, nr_sa=108, nr_nsa=172
    let mut diag_bytes = vec![0u8; 256];
    let lte_bands = vec![1, 3, 7, 28, 48];
    for &b in &lte_bands {
        let bit = b - 1;
        diag_bytes[36 + (bit / 8) as usize] |= 1 << (bit % 8);
    }
    let nr_sa_bands = vec![74, 75, 76, 77, 78, 79];
    for &b in &nr_sa_bands {
        let bit = b - 1;
        diag_bytes[108 + (bit / 8) as usize] |= 1 << (bit % 8);
    }
    let nr_nsa_bands = vec![74, 75, 76, 77, 78, 79];
    for &b in &nr_nsa_bands {
        let bit = b - 1;
        diag_bytes[172 + (bit / 8) as usize] |= 1 << (bit % 8);
    }

    let offsets_default = detect_band_offsets(&diag_bytes, 5);
    assert_eq!(
        offsets_default,
        BandOffsets {
            lte: 36,
            nr_sa: 108,
            nr_nsa: 172
        }
    );

    // Check parsed bands from default layout
    let parsed_lte = parse_bitmask_bands(&diag_bytes, offsets_default.lte, 9, ALL_LTE_BANDS);
    assert_eq!(parsed_lte, lte_bands);

    // 2) Shifted layout offsets: lte=20, nr_sa=50, nr_nsa=80
    let mut diag_shifted = vec![0u8; 256];
    for &b in &lte_bands {
        let bit = b - 1;
        diag_shifted[20 + (bit / 8) as usize] |= 1 << (bit % 8);
    }
    for &b in &nr_sa_bands {
        let bit = b - 1;
        diag_shifted[50 + (bit / 8) as usize] |= 1 << (bit % 8);
    }
    for &b in &nr_nsa_bands {
        let bit = b - 1;
        diag_shifted[80 + (bit / 8) as usize] |= 1 << (bit % 8);
    }

    let offsets_shifted = detect_band_offsets(&diag_shifted, 5);
    assert_eq!(
        offsets_shifted,
        BandOffsets {
            lte: 20,
            nr_sa: 50,
            nr_nsa: 80
        }
    );
}

#[test]
fn test_import_parser() {
    // Valid import JSON
    let valid_json = r#"{
        "sim0": {
            "/nv/item_files/modem/mmode/": {
                "lte_bandpref": {
                    "op": "w",
                    "data": "01020304"
                },
                "other_item": {
                    "op": "d"
                }
            }
        }
    }"#;
    let cmds = parse_import_json(valid_json).expect("Valid import should parse");
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0].slot, 0);
    assert_eq!(cmds[0].op, "w");
    assert_eq!(cmds[0].path, "/nv/item_files/modem/mmode/lte_bandpref");
    assert_eq!(cmds[0].bytes, Some("01020304".to_string()));
    assert_eq!(cmds[1].op, "d");
    assert_eq!(cmds[1].bytes, None);

    // Unknown top-level key
    let unknown_key_json = r#"{
        "sim0": {},
        "invalid_sim_key": {}
    }"#;
    let err = parse_import_json(unknown_key_json).unwrap_err();
    assert!(err.contains("Unknown top-level key(s)"));

    // Bad hex
    let bad_hex_json = r#"{
        "sim0": {
            "/nv/item_files/modem/mmode/": {
                "lte_bandpref": {
                    "op": "w",
                    "data": "00GG"
                }
            }
        }
    }"#;
    let err_hex = parse_import_json(bad_hex_json).unwrap_err();
    assert!(err_hex.contains("Hex string contains non-hex characters"));

    // Odd length
    let odd_hex_json = r#"{
        "sim0": {
            "/nv/item_files/modem/mmode/": {
                "lte_bandpref": {
                    "op": "w",
                    "data": "00G"
                }
            }
        }
    }"#;
    assert!(parse_import_json(odd_hex_json).is_err());
}

#[test]
fn test_feature_is_disabled_predicates() {
    let find_feat = |id: &str| ALL_FEATURES.iter().find(|f| f.id == id).unwrap();

    // r17_2t2t: all 0s -> disabled
    let f_r17 = find_feat("r17_2t2t");
    let b_r17_disabled = vec![0u8; 24];
    let b_r17_enabled = vec![1u8; 24];
    assert!((f_r17.is_disabled_fn)(&[&b_r17_disabled]));
    assert!(!(f_r17.is_disabled_fn)(&[&b_r17_enabled]));

    // r16_2t1t: both paths all 0s -> disabled
    let f_r16 = find_feat("r16_2t1t");
    let b1 = vec![0u8; 17];
    let b2 = vec![0u8; 3];
    assert!((f_r16.is_disabled_fn)(&[&b1, &b2]));
    assert!(!(f_r16.is_disabled_fn)(&[&vec![1u8; 17], &b2]));

    // ul_mimo: [1, 1, 0] -> disabled
    let f_ul_mimo = find_feat("ul_mimo");
    assert!((f_ul_mimo.is_disabled_fn)(&[&[1, 1, 0, 0, 0]]));
    assert!(!(f_ul_mimo.is_disabled_fn)(&[&[0, 0, 0, 0, 0]]));

    // fdd_ul_mimo: [0, 0] -> disabled
    let f_fdd = find_feat("fdd_ul_mimo");
    assert!((f_fdd.is_disabled_fn)(&[&[0, 0]]));
    assert!(!(f_fdd.is_disabled_fn)(&[&[1, 0]]));

    // nr_ulca
    let f_nr_ulca = find_feat("nr_ulca");
    let b_ulca_dis = vec![
        vec![0u8],
        vec![1, 1, 1, 1, 0, 0],
        vec![0, 1, 1],
        vec![0, 1, 1, 0, 1, 1],
        vec![0, 0],
    ];
    let b_ulca_refs: Vec<&[u8]> = b_ulca_dis.iter().map(|v| v.as_slice()).collect();
    assert!((f_nr_ulca.is_disabled_fn)(&b_ulca_refs));

    // dl_nrca: [1] -> disabled
    let f_dl = find_feat("dl_nrca");
    assert!((f_dl.is_disabled_fn)(&[&[1]]));
    assert!(!(f_dl.is_disabled_fn)(&[&[0]]));

    // lowband_4rx
    let f_low = find_feat("lowband_4rx");
    let b_low_dis = vec![
        0, 0, 0, 5, 8, 0, 0, 2, 20, 0, 0, 2, 26, 0, 0, 2, 28, 0, 0, 2, 71, 0, 0, 2, 0, 0, 0, 0,
    ];
    assert!((f_low.is_disabled_fn)(&[&b_low_dis]));

    // nsa_tf_nrca: b0=[0], b1=[7]
    let f_tf = find_feat("nsa_tf_nrca");
    assert!((f_tf.is_disabled_fn)(&[&[0], &[7]]));

    // nsa_ff_nrca: [0]
    let f_ff = find_feat("nsa_ff_nrca");
    assert!((f_ff.is_disabled_fn)(&[&[0]]));

    // nsa_tt_nrca: b0=[0,0,0], b1=[0,0]
    let f_tt = find_feat("nsa_tt_nrca");
    assert!((f_tt.is_disabled_fn)(&[&[0, 0, 0], &[0, 0]]));

    // segmentation: [0]
    let f_seg = find_feat("segmentation");
    assert!((f_seg.is_disabled_fn)(&[&[0]]));

    // dss: [0, 0]
    let f_dss = find_feat("dss");
    assert!((f_dss.is_disabled_fn)(&[&[0, 0]]));
}

#[test]
fn test_backup_roundtrip() {
    let tmp_dir = std::env::temp_dir().join(format!("mtbtest_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp_dir);
    env::set_var("MTBTOOL_DIR", tmp_dir.to_str().unwrap());

    let entries = vec![
        BackupEntry {
            slot: 0,
            path: "/nv/item_files/modem/mmode/lte_bandpref".to_string(),
            bytes: Some("01020304".to_string()),
        },
        BackupEntry {
            slot: 1,
            path: "/nv/item_files/modem/mmode/nr_band_pref".to_string(),
            bytes: None,
        },
    ];

    let created = create_backup("test_reason", entries.clone()).expect("Create backup should work");

    let list = list_backups().expect("List backups should work");
    assert_eq!(list.len(), 1);

    let retrieved = get_backup(&created.id).expect("Get backup by id should work");
    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.entries, entries);

    let latest = get_backup("latest").expect("Get backup latest should work");
    assert_eq!(latest.id, created.id);

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_cells_parse() {
    // Valid LTE line
    let valid_lte_raw = "ASDIV DATA: earfcn: 300, pci: 12, rsrp_rx0: -95.0, rsrq_rx0: -10.0, rssi_rx0: -70.0, snr_rx0: 15.0";
    let lte_cell = parse_lte_cell(valid_lte_raw, "PCC").expect("Valid LTE cell should parse");
    assert_eq!(
        lte_cell,
        LteCellData {
            label: "PCC".to_string(),
            earfcn: 300,
            pci: 12,
            rsrp: -95.0,
            rsrq: -10.0,
            rssi: -70.0,
            snr: 15.0,
        }
    );

    // Invalid float sentinel 65535.0 (>= 65534.5)
    let invalid_lte_raw = "ASDIV DATA: earfcn: 300, pci: 12, rsrp_rx0: 65535.0, rsrq_rx0: -10.0, rssi_rx0: -70.0, snr_rx0: 15.0";
    assert!(parse_lte_cell(invalid_lte_raw, "PCC").is_none());

    // Garbage line
    let garbage_raw = "SOMETHING ELSE UNRELATED";
    assert!(parse_lte_cell(garbage_raw, "PCC").is_none());

    // Valid NR line
    let valid_nr_raw = "ASDIV DATA: rsrp_rx0: -90.0, rsrq: -11.0";
    let nr_cell = parse_nr_cell(valid_nr_raw, "PCC").expect("Valid NR cell should parse");
    assert_eq!(
        nr_cell,
        NrCellData {
            label: "PCC".to_string(),
            rsrp: -90.0,
            rsrq: -11.0,
        }
    );

    // TX Power valid
    let tx_valid = "TX INFO: tx_power = 18";
    assert_eq!(parse_tx_power(tx_valid), Some(18));

    // TX Power sentinel 65535 -> None
    let tx_sentinel = "TX INFO: tx_power = 65535";
    assert_eq!(parse_tx_power(tx_sentinel), None);
}
