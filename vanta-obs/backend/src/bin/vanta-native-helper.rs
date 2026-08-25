use std::{
    io::{self, BufRead, Write},
    process,
};

use serde_json::json;

const PROTOCOL_VERSION: &str = "vanta-native-helper.v1";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        process::exit(2);
    };
    match command.as_str() {
        "--handshake" => {
            let helper_kind = args.next().unwrap_or_else(|| "capture".to_string());
            let endpoint = std::env::var("VANTA_NATIVE_HELPER_ENDPOINT")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "stdio://vanta-native-helper".to_string());
            println!(
                "{}",
                json!({
                    "helper_kind": helper_kind,
                    "protocol_version": PROTOCOL_VERSION,
                    "process_id": process::id(),
                    "endpoint": endpoint,
                    "health_json": {
                        "state": "ready",
                        "protocol_version": PROTOCOL_VERSION,
                        "degraded": false
                    }
                })
            );
        }
        "--heartbeat" => {
            println!(
                "{}",
                json!({
                    "status": "ready",
                    "health_json": {
                        "state": "ready",
                        "protocol_version": PROTOCOL_VERSION,
                        "degraded": false
                    }
                })
            );
        }
        "--command" => {
            let command_kind = args.next().unwrap_or_else(|| "heartbeat".to_string());
            let payload = args
                .next()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                .unwrap_or_else(|| json!({}));
            println!("{}", command_response(&command_kind, payload));
        }
        "--serve-stdio" => {
            let helper_kind = args.next().unwrap_or_else(|| "capture".to_string());
            let endpoint = std::env::var("VANTA_NATIVE_HELPER_ENDPOINT")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "stdio://vanta-native-helper".to_string());
            println!(
                "{}",
                json!({
                    "helper_kind": helper_kind,
                    "protocol_version": PROTOCOL_VERSION,
                    "process_id": process::id(),
                    "endpoint": endpoint,
                    "health_json": {
                        "state": "ready",
                        "protocol_version": PROTOCOL_VERSION,
                        "transport": "stdio",
                        "lifecycle": "long_lived",
                        "degraded": false
                    }
                })
            );
            io::stdout().flush()?;
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let payload = serde_json::from_str::<serde_json::Value>(&line)?;
                let command_kind = payload
                    .get("command_kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("heartbeat")
                    .to_string();
                let detail = payload
                    .get("payload_json")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                println!("{}", command_response(&command_kind, detail));
                io::stdout().flush()?;
                if command_kind == "shutdown" || command_kind == "report_crash" {
                    break;
                }
            }
        }
        _ => {
            print_usage();
            process::exit(2);
        }
    }
    Ok(())
}

fn print_usage() {
    eprintln!(
        "usage: vanta-native-helper --handshake <capture|encode|replay|audio> | --serve-stdio <capture|encode|replay|audio> | --heartbeat | --command <kind> <payload-json>"
    );
}

fn command_response(command_kind: &str, payload: serde_json::Value) -> serde_json::Value {
    let status = match command_kind {
        "shutdown" => "stopped",
        "report_crash" => "crashed",
        "report_degraded" => "degraded",
        _ => "ready",
    };
    json!({
        "status": status,
        "health_json": {
            "state": status,
            "protocol_version": PROTOCOL_VERSION,
            "transport": "stdio",
            "lifecycle": "long_lived",
            "command": command_kind,
            "detail": payload,
            "degraded": status == "degraded"
        }
    })
}
