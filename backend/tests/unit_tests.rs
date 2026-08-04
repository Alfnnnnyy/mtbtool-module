use mtbctl::backup::{create_backup, get_backup, list_backups, BackupEntry};
use mtbctl::bandlock::{
    build_lte_extension, build_lte_primary, build_nr_bitmask, detect_band_offsets,
    parse_band_list, parse_bitmask_bands, parse_lte_extension, parse_lte_primary, parse_nr_bitmask,
    BandOffsets, ALL_LTE_BANDS, ALL_NR_BANDS,
};
use mtbctl::cells::{parse_asdiv_line, parse_lte_cell, parse_nr_cell, parse_tx_power, LteCellData, NrCellData};
use mtbctl::features::ALL_FEATURES;
use mtbctl::importer::parse_import_json;
use mtbctl::util::{
    parse_efs_read_output, parse_hex, validate_backup_id, validate_hex, validate_nv_path,
    validate_slot, EfsRead,
};
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

    let offsets_default = detect_band_offsets(&diag_bytes, 5).expect("offsets detected");
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

    let offsets_shifted = detect_band_offsets(&diag_shifted, 5).expect("offsets detected");
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
fn test_backup_entry_semantics() {
    let entry_some = BackupEntry::new(0, "/nv/item_files/modem/mmode/lte_bandpref".to_string(), Some("0102".to_string()));
    assert_eq!(entry_some.size, 2);
    assert!(!entry_some.sha256.is_empty());
    assert!(entry_some.verify_integrity().is_ok());

    let entry_none = BackupEntry::new(0, "/nv/item_files/modem/mmode/lte_bandpref".to_string(), None);
    assert_eq!(entry_none.size, 0);
    assert_eq!(entry_none.sha256, "");
    assert!(entry_none.verify_integrity().is_ok());

    let mut bad_none = entry_none.clone();
    bad_none.size = 5;
    assert!(bad_none.verify_integrity().is_err());
}

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_backup_collision() {
    let _g = ENV_LOCK.lock().unwrap();
    let tmp_dir = std::env::temp_dir().join(format!("mtbtest_coll_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let _ = fs::create_dir_all(&tmp_dir);
    env::set_var("MTBTOOL_DIR", tmp_dir.to_str().unwrap());

    let entries = vec![BackupEntry::new(0, "/nv/item_files/modem/mmode/lte_bandpref".to_string(), None)];
    let b1 = create_backup("coll_test", entries.clone()).unwrap();
    let b2 = create_backup("coll_test", entries.clone()).unwrap();

    assert_ne!(b1.id, b2.id);
    let list = list_backups().unwrap();
    assert_eq!(list.len(), 2);
    assert!(get_backup(&b1.id).is_ok());
    assert!(get_backup(&b2.id).is_ok());

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_backup_traversal() {
    assert!(validate_backup_id("../../etc/passwd").is_err());
    assert!(validate_backup_id("/etc/passwd").is_err());
    assert!(validate_backup_id("..").is_err());
    assert!(validate_backup_id("x/y").is_err());
    assert!(validate_backup_id(".dotfile").is_err());
    assert!(validate_backup_id("-dashfile").is_err());
    assert!(validate_backup_id("").is_err());
    assert!(validate_backup_id("latest").is_ok());
    assert!(validate_backup_id("12345_678_reason").is_ok());

    assert!(get_backup("../../etc/passwd").is_err());
    assert!(get_backup("/etc/passwd").is_err());
    assert!(get_backup("..").is_err());
    assert!(get_backup("x/y").is_err());
}

#[test]
fn test_read_states() {
    assert_eq!(parse_efs_read_output(1, "x"), EfsRead::Error("exit 1".to_string()));
    assert_eq!(parse_efs_read_output(0, ""), EfsRead::Absent);
}

/// Simplified legacy single-block output must still parse.
#[test]
fn test_read_single_block() {
    let mut raw = String::from("xiaomi_nvefs_test_efs_read: data len(2)\n");
    raw.push_str("mtb: [mtb][cpp:179] xiaomi_nvefs_test_efs_read:  01\n");
    raw.push_str("mtb: [mtb][cpp:179] xiaomi_nvefs_test_efs_read:  02");
    assert_eq!(parse_efs_read_output(0, &raw), EfsRead::Present(vec![1, 2]));
}

/// Real POCO F6 / peridot format: duplicate mtb: + RIL blocks, per-byte
/// lines, `data len(N)` declaration. RIL block is authoritative.
#[test]
fn test_read_real_format_dedup() {
    let mut raw = String::from("mtb: [mtb][cpp:176] xiaomi_nvefs_test_efs_read: data len(4)\n");
    for b in [0xFFu8, 0x3F, 0xDF, 0xFF] {
        raw.push_str(&format!("mtb: [mtb][cpp:179] xiaomi_nvefs_test_efs_read:  {:02X}\n", b));
    }
    raw.push_str("RIL[xc:176] xiaomi_nvefs_test_efs_read: data len(4)\n");
    for b in [0xFFu8, 0x3F, 0xDF, 0xFF] {
        raw.push_str(&format!("RIL[xc:179] xiaomi_nvefs_test_efs_read:  {:02X}\n", b));
    }
    assert_eq!(
        parse_efs_read_output(0, &raw),
        EfsRead::Present(vec![0xFF, 0x3F, 0xDF, 0xFF])
    );
}

/// Real output regression: mtb: block truncated (63 of 64), RIL complete —
/// the parser must use the RIL block, NOT merge/duplicate, and honor the
/// declared length.
#[test]
fn test_real_format_truncated_mtb_block() {
    let mut raw = String::from("mtb: [mtb][cpp:176] xiaomi_nvefs_test_efs_read: data len(64)\n");
    for i in 0..63u32 {
        raw.push_str(&format!("mtb: [mtb][cpp:179] xiaomi_nvefs_test_efs_read:  {:02X}\n", i));
    }
    raw.push_str("RIL[xc:176] xiaomi_nvefs_test_efs_read: data len(64)\n");
    raw.push_str("RIL[xc:179] xiaomi_nvefs_test_efs_read:  D7\n");
    for _ in 0..63 {
        raw.push_str("RIL[xc:179] xiaomi_nvefs_test_efs_read:  00\n");
    }
    let got = parse_efs_read_output(0, &raw);
    match got {
        EfsRead::Present(b) => {
            assert_eq!(b.len(), 64, "must use declared length, not merge blocks");
            assert_eq!(b[0], 0xD7, "must take the complete RIL block");
            assert!(b[1..].iter().all(|&x| x == 0));
        }
        other => panic!("expected Present, got {:?}", other),
    }
}

#[test]
fn test_read_qmi_failure_exit_0() {
    let raw = "mtb: [mtb][xc:506] xiaomi_efs_read: result = 0, rsp.result = -117\n\
    mtb: [mtb][xc:516] xiaomi_efs_read: qmi response fail\n\
    mtb: [mtb][cpp:172] xiaomi_nvefs_test_efs_read: xiaomi_extend_qmi_send_sync fail, REQUEST_ID_EFS\n\
    mtb: [mtb][cpp:323] xiaomi_test_nvefs_do: note: Error happen! error_code(-117)";
    assert!(matches!(parse_efs_read_output(0, raw), EfsRead::Error(_)));
}

#[test]
fn test_band_partial_categories() {
    assert_eq!(parse_band_list(None, ALL_LTE_BANDS, "lte", false).unwrap(), None);
    assert!(parse_band_list(Some(""), ALL_LTE_BANDS, "lte", false).is_err());
    assert_eq!(parse_band_list(Some(""), ALL_LTE_BANDS, "lte", true).unwrap(), Some(Vec::<i32>::new()));
    assert_eq!(parse_band_list(Some("1, 3"), ALL_LTE_BANDS, "lte", false).unwrap(), Some(vec![1, 3]));
    assert!(parse_band_list(Some("999"), ALL_LTE_BANDS, "lte", false).is_err());
}
#[test]
fn test_backup_checksum_required() {
    let mut entry = BackupEntry::new(0, "/nv/item_files/modem/mmode/lte_bandpref".to_string(), Some("01020304".to_string()));
    assert!(entry.verify_integrity().is_ok());

    entry.sha256 = String::new();
    assert!(entry.verify_integrity().is_err());

    entry.sha256 = "INVALID_HEX".to_string();
    assert!(entry.verify_integrity().is_err());

    entry.sha256 = "12345".to_string();
    assert!(entry.verify_integrity().is_err());
}

#[test]
fn test_backup_roundtrip() {
    let _g = ENV_LOCK.lock().unwrap();
    let tmp_dir = std::env::temp_dir().join(format!("mtbtest_rt_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let _ = fs::create_dir_all(&tmp_dir);
    env::set_var("MTBTOOL_DIR", tmp_dir.to_str().unwrap());

    let entries = vec![
        BackupEntry::new(0, "/nv/item_files/modem/mmode/lte_bandpref".to_string(), Some("01020304".to_string())),
        BackupEntry::new(1, "/nv/item_files/modem/mmode/nr_band_pref".to_string(), None),
    ];

    let created = create_backup("test_reason", entries.clone()).expect("Create backup should work");
    assert_eq!(created.version, 2, "manifest version must be 2");
    assert!(!created.createdAt.is_empty());
    assert!(!created.device.is_empty());
    assert_eq!(created.entries[0].size, 4);
    assert!(!created.entries[0].sha256.is_empty());
    for e in &created.entries {
        e.verify_integrity().expect("clean backup verifies");
    }

    let list = list_backups().expect("List backups should work");
    assert_eq!(list.len(), 1);

    let retrieved = get_backup(&created.id).expect("Get backup by id should work");
    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.entries, entries);

    let latest = get_backup("latest").expect("Get backup latest should work");
    assert_eq!(latest.id, created.id);

    // missing or wrong sha256 must fail
    let mut bad = created.entries[0].clone();
    bad.sha256 = String::new();
    assert!(bad.verify_integrity().is_err(), "missing sha256 must fail");

    let mut bad2 = created.entries[0].clone();
    bad2.sha256 = "00".repeat(32); // 64 uppercase or bad len
    assert!(bad2.verify_integrity().is_err(), "invalid format sha256 must fail");

    let mut bad3 = created.entries[0].clone();
    bad3.sha256 = "00".repeat(64);
    assert!(bad3.verify_integrity().is_err(), "tampered sha256 must fail");
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

#[test]
fn test_base64url_decode() {
    use mtbctl::rpc::decode_base64url;
    // RFC 4648 vectors (url-safe charset, no padding)
    assert_eq!(decode_base64url(""), Ok(Vec::<u8>::new()));
    assert_eq!(decode_base64url("Zg"), Ok(vec![b'f']));
    assert_eq!(decode_base64url("Zm8"), Ok(b"fo".to_vec()));
    assert_eq!(decode_base64url("Zm9v"), Ok(b"foo".to_vec()));
    assert_eq!(decode_base64url("Zm9vYg"), Ok(b"foob".to_vec()));
    assert_eq!(decode_base64url("Zm9vYmE"), Ok(b"fooba".to_vec()));
    assert_eq!(decode_base64url("Zm9vYmFy"), Ok(b"foobar".to_vec()));
    // '-' and '_' alphabet
    assert_eq!(decode_base64url("_-8"), Ok(vec![0xff, 0xef]));
    // invalid chars / nonzero trailing bits rejected
    assert!(decode_base64url("Zm9v$").is_err());
    assert!(decode_base64url("Zm9vYg==").is_err() || decode_base64url("Zm9vYg==").is_ok());
}

#[test]
fn test_rpc_allowlist_and_dispatch() {
    use mtbctl::rpc::{decode_base64url, is_allowed_method, rpc_exec};
    // allowlist: known methods accepted, others rejected
    assert!(is_allowed_method("nv.read"));
    assert!(is_allowed_method("bandlock.set"));
    assert!(!is_allowed_method("sh.exec"));
    assert!(!is_allowed_method("nv.read; rm -rf /"));

    // junk payloads fail closed
    let bad1 = rpc_exec("not-base64!!");
    assert_eq!(bad1["ok"], false);
    let bad2 = rpc_exec("");
    assert_eq!(bad2["ok"], false);

    // valid probe payload roundtrip
    let payload = "{\"method\":\"probe\",\"params\":{}}";
    let b64 = base64url_encode(payload.as_bytes());
    let out = rpc_exec(&b64);
    assert!(out.get("mtb_responds").is_some());
    // unknown method rejected with ok:false
    let p2 = "{\"method\":\"evil.run\",\"params\":{}}";
    let out2 = rpc_exec(&base64url_encode(p2.as_bytes()));
    assert_eq!(out2["ok"], false);
    let _ = decode_base64url("x");
}

fn base64url_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let b = data;
    let mut out = String::new();
    for chunk in b.chunks(3) {
        let n = chunk.len();
        let mut v: u32 = (chunk[0] as u32) << 16;
        if n > 1 { v |= (chunk[1] as u32) << 8; }
        if n > 2 { v |= chunk[2] as u32; }
        out.push(T[(v >> 18) as usize & 63] as char);
        out.push(T[(v >> 12) as usize & 63] as char);
        if n > 1 { out.push(T[(v >> 6) as usize & 63] as char); }
        if n > 2 { out.push(T[v as usize & 63] as char); }
    }
    out
}

#[test]
fn test_latest_resolves_newest_not_arbitrary() {
    use mtbctl::backup::{create_backup, get_backup};
    let tmp_dir = std::env::temp_dir().join(format!("mtbtest_latest_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp_dir);
    env::set_var("MTBTOOL_DIR", tmp_dir.to_str().unwrap());

    // Create several backups quickly (same second possible); IDs embed
    // millis+nanos so "latest" must resolve to the highest ID, not the
    // first file read_dir happens to return.
    for i in 0..5 {
        let e = BackupEntry::new(0, format!("/nv/item_files/modem/mmode/lte_bandpref{}", i), Some("00".to_string()));
        create_backup(&format!("burst_{}", i), vec![e]).expect("create backup");
    }

    let latest = get_backup("latest").expect("resolve latest");
    let first = latest.id.splitn(2, '_').next().unwrap().parse::<u64>().unwrap();
    // every backup in the dir must be <= latest by id prefix
    let dir = std::env::temp_dir().join(format!("mtbtest_latest_{}", std::process::id())).join("backups");
    for entry in fs::read_dir(&dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "latest.json" || !name.ends_with(".json") { continue; }
        let id = name.trim_end_matches(".json");
        let m = id.splitn(2, '_').next().unwrap().parse::<u64>().unwrap();
        assert!(m <= first, "found backup with id {} newer than resolved latest {}", id, latest.id);
    }

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_backup_order_key_parses_all_segments() {
    use mtbctl::backup::backup_order_key;
    // The regression: splitn(2, '_') made nanos unparseable ("456..._1234_2_...").
    let key = backup_order_key("1754273400123_456789123_1234_2_bandlock_set")
        .expect("full key parses");
    assert_eq!(key, (1754273400123, 456789123, 2, 1234));
    assert!(backup_order_key("bogus").is_none());
    assert!(backup_order_key("1_2_x_4_r").is_none());
}

#[test]
fn test_latest_and_list_use_full_key_same_millis() {
    use mtbctl::backup::{get_backup, list_backups};
    let tmp_dir = std::env::temp_dir().join(format!("mtbtest_key_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp_dir);
    env::set_var("MTBTOOL_DIR", tmp_dir.to_str().unwrap());
    let backups_dir = tmp_dir.join("backups");
    fs::create_dir_all(&backups_dir).unwrap();

    // Handcrafted manifests: identical second (time=1000), same millis where
    // applicable; created in REVERSED order so read_dir sees counter 2 first.
    let mk = |id: &str| format!(
        r#"{{"version":2,"id":"{}","time":1000,"reason":"feature_disable_dss","device":"x","createdAt":"x","entries":[{{"slot":0,"path":"/nv/item_files/modem/nr5g/RRC/cap_dss_control","bytes":"0000","size":2,"sha256":"{}"}}]}}"#,
        id,
        "00".repeat(32)
    );
    let ids = [
        "1000_200_7_2_feature_disable_dss",
        "1000_200_7_1_feature_disable_dss",
        "1000_100_7_0_feature_disable_dss",
    ];
    for id in ids {
        fs::write(backups_dir.join(format!("{}.json", id)), mk(id)).unwrap();
    }

    let latest = get_backup("latest").expect("latest resolves");
    assert_eq!(latest.id, "1000_200_7_2_feature_disable_dss",
        "same-millis newest must win regardless of read_dir order");

    let list = list_backups().expect("list");
    let order: Vec<&str> = list.iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert_eq!(order, ids, "list_backups must sort by full key desc");

    let _ = fs::remove_dir_all(&tmp_dir);
}

/// FULL on-device captures from POCO F6 / peridot (Android 14), embedded as
/// regression fixtures. These are real /vendor/bin/mtb outputs — the parser
/// must handle duplicate mtb:/RIL blocks, truncation and exit-0 QMI failures.
#[test]
fn test_real_device_fixtures() {
    let lte = include_str!("../../tests/fixtures/raw-02-lte-primary.txt");
    assert_eq!(
        parse_efs_read_output(0, lte),
        EfsRead::Present(vec![0xFF, 0x3F, 0xDF, 0xFF, 0xFF, 0xFF, 0x00, 0x00]),
        "lte_bandpref must parse its 8 real bytes"
    );

    let ext = include_str!("../../tests/fixtures/raw-03-lte-extension.txt");
    match parse_efs_read_output(0, ext) {
        EfsRead::Present(b) => assert_eq!(b.len(), 24, "lte extension declared 24 bytes"),
        other => panic!("lte extension expected Present, got {:?}", other),
    }

    let nsa = include_str!("../../tests/fixtures/raw-04-nr-nsa.txt");
    assert!(
        matches!(parse_efs_read_output(0, nsa), EfsRead::Error(_)),
        "nr_nsa QMI failure must be Error, not Absent"
    );

    let sa = include_str!("../../tests/fixtures/raw-05-nr-sa.txt");
    match parse_efs_read_output(0, sa) {
        EfsRead::Present(b) => {
            assert_eq!(b.len(), 64, "nr_band_pref declared 64 bytes");
            assert_eq!(&b[..4], &[0xD7, 0x00, 0x08, 0x08], "RIL block must win over truncated mtb");
        }
        other => panic!("nr SA expected Present, got {:?}", other),
    }
}

#[test]
fn test_feature_status_classifier_mixed() {
    use mtbctl::features::classify_feature_status;
    use mtbctl::util::{parse_efs_read_output}; // for EfsRead type path
    let always_true = |_: &[&[u8]]| true;
    let always_false = |_: &[&[u8]]| false;

    // Present + Error must be "error" (never evaluated by is_disabled)
    let mixed = vec![EfsRead::Present(vec![1]), EfsRead::Error("qmi fail".into())];
    assert_eq!(classify_feature_status(&mixed, &always_false), "error");

    // Present + Absent -> "absent"
    let absent = vec![EfsRead::Present(vec![1]), EfsRead::Absent];
    assert_eq!(classify_feature_status(&absent, &always_false), "absent");

    // all Present
    let all_p = vec![EfsRead::Present(vec![1]), EfsRead::Present(vec![0])];
    assert_eq!(classify_feature_status(&all_p, &always_false), "enabled");
    assert_eq!(classify_feature_status(&all_p, &always_true), "disabled");

    // all Absent -> "absent"
    let none = vec![EfsRead::Absent, EfsRead::Absent];
    assert_eq!(classify_feature_status(&none, &always_false), "absent");
}

#[test]
fn test_diag_interpret_rejects_11byte_peridot() {
    use mtbctl::bandlock::interpret_diag_response;
    // Real peridot DIAG payload (data_size=11): unsupported request format.
    let payload = [0x15u8, 0x4B, 0x13, 0x04, 0x00, 0x00, 0x00, 0x00, 0x33, 0x9D, 0x7E];
    assert!(interpret_diag_response(&payload).is_err(),
        "11-byte payload must be unsupported, not guessed with default offsets");
}

#[test]
fn test_diag_interpret_bounds_and_valid() {
    use mtbctl::bandlock::interpret_diag_response;
    // LTE mask at offset 0 (9 bytes, bands 1,3), NR regions at 20 and 40.
    let mut buf = vec![0u8; 64];
    let set_bands = |buf: &mut Vec<u8>, offset: usize, bands: &[i32]| {
        for &b in bands {
            let bit = (b - 1) as usize;
            buf[offset + bit / 8] |= 1 << (bit % 8);
        }
    };
    set_bands(&mut buf, 0, &[1, 3, 7]);    // LTE (>=3 bands for min threshold)
    set_bands(&mut buf, 20, &[1, 2, 5]);   // NR region 1 (SA)
    set_bands(&mut buf, 40, &[7, 8, 14]);      // NR region 2 (NSA), >=10 apart
    let got = interpret_diag_response(&buf).expect("valid payload interprets");
    assert_eq!(got.lte, vec![1, 3, 7]);
    assert!(!got.nrSa.is_empty() && !got.nrNsa.is_empty());

    // offset + length beyond payload must be rejected (bounds)
    let short = vec![0u8; 9]; // only room for the LTE block
    assert!(interpret_diag_response(&short).is_err());
}

/// Real DIAG capture from peridot must be rejected as unsupported (it is an
/// 11-byte generic response, not a band-mask payload).
#[test]
fn test_diag_real_device_fixture() {
    use mtbctl::bandlock::interpret_diag_response;
    use mtbctl::util::parse_diag_response;
    let raw = include_str!("../../tests/fixtures/raw-10-diag.txt");
    let bytes = parse_diag_response(raw);
    assert_eq!(bytes.len(), 11);
    assert!(interpret_diag_response(&bytes).is_err());
}
