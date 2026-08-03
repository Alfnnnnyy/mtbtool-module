use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::thread;

use crate::backup::{list_backups, restore_backup};
use crate::bandlock::{detect_bandlock, get_bandlock, set_bandlock};
use crate::cells::get_cells;
use crate::config::{get_config, set_config};
use crate::features::{check_features, disable_feature, restore_feature};
use crate::importer::{import_apply, import_preview};
use crate::mtb::exec_mtb;
use crate::nv::{delete_nv, read_nv, write_nv};

pub fn dispatch_cmd(cmd: &str, args: &Value) -> Value {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let base = parts
        .get(0..2)
        .map(|s| s.join(" "))
        .unwrap_or_else(|| cmd.to_string());

    // Frontend bridge sends all args as strings — accept number or string.
    let slot = args
        .get("slot")
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())))
        .map(|v| v as i32)
        .unwrap_or(0);

    match base.as_str() {
        "probe" => {
            let (code, _) = exec_mtb(&["0"]);
            let mtb_bin = crate::mtb::get_mtb_bin();
            let mtb_exists = std::path::Path::new(&mtb_bin).exists();

            let model = exec_mtb_getprop("ro.product.model");
            let sdk = exec_mtb_getprop("ro.build.version.sdk");

            json!({
                "ok": true,
                "mtb_path": mtb_bin,
                "mtb_exists": mtb_exists,
                "mtb_executable": code == 0,
                "mtbctl_version": "1.0.0",
                "model": model,
                "android_sdk": sdk,
                "data_dir": crate::mtb::get_mtbtool_dir().to_string_lossy()
            })
        }
        "nv read" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            read_nv(path, slot)
        }
        "nv write" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let hex = args.get("hex").and_then(|v| v.as_str()).unwrap_or("");
            let reason = args.get("reason").and_then(|v| v.as_str());
            write_nv(path, hex, slot, reason)
        }
        "nv delete" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let reason = args.get("reason").and_then(|v| v.as_str());
            delete_nv(path, slot, reason)
        }
        "bandlock get" => get_bandlock(slot),
        "bandlock set" => {
            let lte = args.get("lte").and_then(|v| v.as_str());
            let nr_nsa = args.get("nrNsa").and_then(|v| v.as_str());
            let nr_sa = args.get("nrSa").and_then(|v| v.as_str());
            set_bandlock(slot, lte, nr_nsa, nr_sa)
        }
        "bandlock detect" => detect_bandlock(slot),
        "features check" => check_features(slot),
        "features disable" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            disable_feature(id, slot)
        }
        "features restore" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            restore_feature(id, slot)
        }
        "cells get" => get_cells(slot),
        "modem restart" => {
            let (code, _) = exec_mtb(&["11", "0"]);
            json!({ "ok": code == 0, "exit": code })
        }
        "import preview" => {
            let json_str = if let Some(s) = args.get("json").and_then(|v| v.as_str()) {
                s.to_string()
            } else {
                args.to_string()
            };
            import_preview(&json_str)
        }
        "import apply" => {
            let json_str = if let Some(s) = args.get("json").and_then(|v| v.as_str()) {
                s.to_string()
            } else {
                args.to_string()
            };
            import_apply(&json_str)
        }
        "backup restore" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .or_else(|| parts.get(2).copied())
                .unwrap_or("latest");
            match restore_backup(id) {
                Ok(restored) => json!({ "ok": true, "restored": restored }),
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }
        "config get" => get_config(),
        "config set" => {
            let json_str = if let Some(s) = args.get("json").and_then(|v| v.as_str()) {
                s.to_string()
            } else {
                args.to_string()
            };
            set_config(&json_str)
        }
        _ => json!({ "ok": false, "error": format!("Unknown command: {}", cmd) }),
    }
}

fn exec_mtb_getprop(prop: &str) -> String {
    let output = std::process::Command::new("getprop")
        .arg(prop)
        .output();
    match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                "unknown".to_string()
            } else {
                s
            }
        }
        Err(_) => "unknown".to_string(),
    }
}

pub fn run_server(port: u16) -> std::io::Result<()> {
    let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port);
    let listener = TcpListener::bind(addr)?;
    eprintln!("mtbctl serve listening on http://127.0.0.1:{}", port);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let _ = handle_connection(stream);
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut req_line = String::new();
    if reader.read_line(&mut req_line)? == 0 {
        return Ok(());
    }

    let parts: Vec<&str> = req_line.split_whitespace().collect();
    if parts.len() < 2 {
        return send_response(&mut stream, 400, json!({"ok": false, "error": "Bad request"}));
    }

    let method = parts[0];
    let path = parts[1];

    let mut content_len: usize = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_lowercase();
        if lower.starts_with("content-length:") {
            if let Some(val) = line.split(':').nth(1) {
                content_len = val.trim().parse().unwrap_or(0);
            }
        }
    }

    if content_len > 65536 {
        return send_response(&mut stream, 413, json!({"ok": false, "error": "Body exceeds 64KB cap"}));
    }

    if method == "GET" && path == "/health" {
        return send_response(&mut stream, 200, json!({"ok": true}));
    }

    if method == "POST" && path == "/api" {
        let mut body_bytes = vec![0u8; content_len];
        reader.read_exact(&mut body_bytes)?;
        let body_str = String::from_utf8_lossy(&body_bytes);

        let req_json: Value = match serde_json::from_str(&body_str) {
            Ok(v) => v,
            Err(e) => return send_response(&mut stream, 400, json!({"ok": false, "error": format!("Invalid JSON body: {}", e)})),
        };

        let cmd = req_json.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
        let args = req_json.get("args").cloned().unwrap_or(json!({}));

        let res_json = dispatch_cmd(cmd, &args);
        return send_response(&mut stream, 200, res_json);
    }

    send_response(&mut stream, 404, json!({"ok": false, "error": "Not found"}))
}

fn send_response(stream: &mut TcpStream, code: u16, body: Value) -> std::io::Result<()> {
    let body_str = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_string());
    let status_text = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Internal Error",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        code, status_text, body_str.len(), body_str
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}
