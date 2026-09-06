//! Bundled SQL language provider for statement and procedural-block folding.
//!
//! The plugin owns only protocol handling; SQL lexing and fold-region
//! detection live in the shared core so tests and future providers can reuse it.

use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            continue;
        };
        match msg["event"].as_str().unwrap_or("") {
            "init" => register_language_provider(&mut stdout.lock()),
            "on_file_open" => {
                if let Some(path) = msg["path"].as_str() {
                    handle_open(path, &mut stdout.lock());
                }
            }
            "on_quit" | "shutdown" => break,
            _ => {}
        }
    }
}

fn register_language_provider(out: &mut impl Write) {
    let msg = serde_json::json!({
        "event": "action",
        "action": "register_language_provider",
        "params": { "extensions": ["sql"], "capabilities": ["fold"] }
    });
    let _ = writeln!(out, "{msg}");
    let _ = out.flush();
}

fn handle_open(path: &str, out: &mut impl Write) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let regions = mantis::fold_detectors::sql_fold(&content);
    let regions: Vec<Vec<usize>> = regions.iter().map(|r| vec![r.start, r.end]).collect();
    let msg = serde_json::json!({
        "event": "action",
        "action": "set_fold_regions",
        "params": { "path": path, "regions": regions }
    });
    let _ = writeln!(out, "{msg}");
    let _ = out.flush();
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
