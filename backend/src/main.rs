use serde_json::{json, Value};
use std::env;
use std::fs;

use mtbctl::http;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: mtbctl <command> [args]");
        std::process::exit(1);
    }

    let cmd_arg = &args[1];

    let result = match cmd_arg.as_str() {
        "probe" => http::dispatch_cmd("probe", &json!({})),
        "nv" => parse_nv_cmd(&args[2..]),
        "bandlock" => parse_bandlock_cmd(&args[2..]),
        "features" => parse_features_cmd(&args[2..]),
        "cells" => parse_cells_cmd(&args[2..]),
        "modem" => parse_modem_cmd(&args[2..]),
        "import" => parse_import_cmd(&args[2..]),
        "backup" => parse_backup_cmd(&args[2..]),
        "config" => parse_config_cmd(&args[2..]),
        "serve" => parse_serve_cmd(&args[2..]),
        _ => {
            eprintln!("Unknown command group: {}", cmd_arg);
            std::process::exit(1);
        }
    };

    println!("{}", serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()));
}

fn parse_slot(args: &[String]) -> (i32, Vec<String>) {
    let mut slot = 0i32;
    let mut remaining = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--slot" && i + 1 < args.len() {
            if let Ok(s) = args[i + 1].parse::<i32>() {
                slot = s;
            }
            i += 2;
        } else {
            remaining.push(args[i].clone());
            i += 1;
        }
    }
    (slot, remaining)
}

fn parse_nv_cmd(args: &[String]) -> Value {
    if args.is_empty() {
        return json!({ "ok": false, "error": "Missing nv subcommand" });
    }

    let sub = &args[0];
    let (slot, rem) = parse_slot(&args[1..]);

    match sub.as_str() {
        "read" => {
            if rem.is_empty() {
                return json!({ "ok": false, "error": "Usage: mtbctl nv read <path> [--slot N]" });
            }
            http::dispatch_cmd("nv read", &json!({ "path": rem[0], "slot": slot }))
        }
        "write" => {
            if rem.len() < 2 {
                return json!({ "ok": false, "error": "Usage: mtbctl nv write <path> <hex> [--slot N] [--reason R]" });
            }
            let path = &rem[0];
            let hex = &rem[1];
            let mut reason = None;
            let mut i = 2;
            while i < rem.len() {
                if rem[i] == "--reason" && i + 1 < rem.len() {
                    reason = Some(rem[i + 1].as_str());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            http::dispatch_cmd(
                "nv write",
                &json!({ "path": path, "hex": hex, "slot": slot, "reason": reason }),
            )
        }
        "delete" => {
            if rem.is_empty() {
                return json!({ "ok": false, "error": "Usage: mtbctl nv delete <path> [--slot N] [--reason R]" });
            }
            let path = &rem[0];
            let mut reason = None;
            let mut i = 1;
            while i < rem.len() {
                if rem[i] == "--reason" && i + 1 < rem.len() {
                    reason = Some(rem[i + 1].as_str());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            http::dispatch_cmd(
                "nv delete",
                &json!({ "path": path, "slot": slot, "reason": reason }),
            )
        }
        _ => json!({ "ok": false, "error": format!("Unknown nv subcommand: {}", sub) }),
    }
}

fn parse_bandlock_cmd(args: &[String]) -> Value {
    if args.is_empty() {
        return json!({ "ok": false, "error": "Missing bandlock subcommand" });
    }

    let sub = &args[0];
    let (slot, rem) = parse_slot(&args[1..]);

    match sub.as_str() {
        "get" => http::dispatch_cmd("bandlock get", &json!({ "slot": slot })),
        "set" => {
            let mut lte = None;
            let mut nr_nsa = None;
            let mut nr_sa = None;

            let mut i = 0;
            while i < rem.len() {
                if rem[i] == "--lte" && i + 1 < rem.len() {
                    lte = Some(rem[i + 1].as_str());
                    i += 2;
                } else if rem[i] == "--nrNsa" && i + 1 < rem.len() {
                    nr_nsa = Some(rem[i + 1].as_str());
                    i += 2;
                } else if rem[i] == "--nrSa" && i + 1 < rem.len() {
                    nr_sa = Some(rem[i + 1].as_str());
                    i += 2;
                } else {
                    i += 1;
                }
            }

            http::dispatch_cmd(
                "bandlock set",
                &json!({
                    "slot": slot,
                    "lte": lte,
                    "nrNsa": nr_nsa,
                    "nrSa": nr_sa
                }),
            )
        }
        "detect" => http::dispatch_cmd("bandlock detect", &json!({ "slot": slot })),
        _ => json!({ "ok": false, "error": format!("Unknown bandlock subcommand: {}", sub) }),
    }
}

fn parse_features_cmd(args: &[String]) -> Value {
    if args.is_empty() {
        return json!({ "ok": false, "error": "Missing features subcommand" });
    }

    let sub = &args[0];
    let (slot, rem) = parse_slot(&args[1..]);

    match sub.as_str() {
        "check" => http::dispatch_cmd("features check", &json!({ "slot": slot })),
        "disable" => {
            if rem.is_empty() {
                return json!({ "ok": false, "error": "Usage: mtbctl features disable <id> [--slot N]" });
            }
            http::dispatch_cmd("features disable", &json!({ "id": rem[0], "slot": slot }))
        }
        "restore" => {
            if rem.is_empty() {
                return json!({ "ok": false, "error": "Usage: mtbctl features restore <id> [--slot N]" });
            }
            http::dispatch_cmd("features restore", &json!({ "id": rem[0], "slot": slot }))
        }
        _ => json!({ "ok": false, "error": format!("Unknown features subcommand: {}", sub) }),
    }
}

fn parse_cells_cmd(args: &[String]) -> Value {
    if args.is_empty() {
        return json!({ "ok": false, "error": "Missing cells subcommand" });
    }

    let sub = &args[0];
    let (slot, _) = parse_slot(&args[1..]);

    match sub.as_str() {
        "get" => http::dispatch_cmd("cells get", &json!({ "slot": slot })),
        _ => json!({ "ok": false, "error": format!("Unknown cells subcommand: {}", sub) }),
    }
}

fn parse_modem_cmd(args: &[String]) -> Value {
    if args.is_empty() || args[0] != "restart" {
        return json!({ "ok": false, "error": "Usage: mtbctl modem restart" });
    }
    http::dispatch_cmd("modem restart", &json!({}))
}

fn parse_json_input(args: &[String]) -> Result<String, String> {
    let mut json_str = None;
    let mut file_path = None;

    let mut i = 0;
    while i < args.len() {
        if args[i] == "--json" && i + 1 < args.len() {
            json_str = Some(args[i + 1].clone());
            i += 2;
        } else if args[i] == "--file" && i + 1 < args.len() {
            file_path = Some(args[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }

    if let Some(s) = json_str {
        Ok(s)
    } else if let Some(p) = file_path {
        fs::read_to_string(&p).map_err(|e| format!("Failed to read file {}: {}", p, e))
    } else {
        Err("Expected --json <s> or --file <path>".to_string())
    }
}

fn parse_import_cmd(args: &[String]) -> Value {
    if args.is_empty() {
        return json!({ "ok": false, "error": "Missing import subcommand" });
    }

    let sub = &args[0];
    let json_input = match parse_json_input(&args[1..]) {
        Ok(s) => s,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    match sub.as_str() {
        "preview" => http::dispatch_cmd("import preview", &json!({ "json": json_input })),
        "apply" => http::dispatch_cmd("import apply", &json!({ "json": json_input })),
        _ => json!({ "ok": false, "error": format!("Unknown import subcommand: {}", sub) }),
    }
}

fn parse_backup_cmd(args: &[String]) -> Value {
    if args.is_empty() {
        return json!({ "ok": false, "error": "Missing backup subcommand" });
    }

    let sub = &args[0];
    match sub.as_str() {
        "list" => http::dispatch_cmd("backup list", &json!({})),
        "restore" => {
            let id = args.get(1).map(|s| s.as_str()).unwrap_or("latest");
            http::dispatch_cmd("backup restore", &json!({ "id": id }))
        }
        _ => json!({ "ok": false, "error": format!("Unknown backup subcommand: {}", sub) }),
    }
}

fn parse_config_cmd(args: &[String]) -> Value {
    if args.is_empty() {
        return json!({ "ok": false, "error": "Missing config subcommand" });
    }

    let sub = &args[0];
    match sub.as_str() {
        "get" => http::dispatch_cmd("config get", &json!({})),
        "set" => {
            let json_input = match parse_json_input(&args[1..]) {
                Ok(s) => s,
                Err(e) => return json!({ "ok": false, "error": e }),
            };
            http::dispatch_cmd("config set", &json!({ "json": json_input }))
        }
        _ => json!({ "ok": false, "error": format!("Unknown config subcommand: {}", sub) }),
    }
}

fn parse_serve_cmd(args: &[String]) -> Value {
    let mut port = 28082u16;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            if let Ok(p) = args[i + 1].parse::<u16>() {
                port = p;
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    if let Err(e) = http::run_server(port) {
        json!({ "ok": false, "error": format!("Server error: {}", e) })
    } else {
        json!({ "ok": true })
    }
}
