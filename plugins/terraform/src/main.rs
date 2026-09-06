//! Bundled Terraform/HCL language provider plugin for mantis.
//!
//! Implements the mantis plugin protocol to provide language services for
//! `.tf`, `.tfvars` and `.hcl` files (HashiCorp HCL — Terraform, TFLint,
//! OpenBao, Nomad, Consul). Today, it registers the `fold` capability and
//! responds to `on_file_open` events by running the shared `hcl_brace_fold`
//! detector — a brace-nesting detector that skips `#`, `//`, and `/* … */`
//! comments, double-quoted strings, and heredocs — and returning the fold
//! regions.

use std::io::{self, BufRead, Write};
use std::path::Path;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event = msg["event"].as_str().unwrap_or("");
        match event {
            "init" => {
                register_language_provider(&mut stdout.lock());
            }
            "on_file_open" => {
                if let Some(path) = msg["path"].as_str() {
                    handle_file_open(path, &mut stdout.lock());
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
        "params": {
            "extensions": ["tf", "tfvars", "hcl"],
            "capabilities": ["fold"]
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

fn handle_file_open(path_str: &str, out: &mut impl Write) {
    let path = Path::new(path_str);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !fold_regions_ok(ext) {
        return;
    }
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let regions = mantis::fold_detectors::hcl_brace_fold(&src);
    send_set_fold_regions(&regions, path_str, out);
}

/// Returns `true` when files with this extension are handled by the provider
/// (`.tf`, `.tfvars`, `.hcl`). Kept separate so the guard is unit-testable
/// without touching the filesystem.
fn fold_regions_ok(ext: &str) -> bool {
    matches!(ext, "tf" | "tfvars" | "hcl")
}

fn send_set_fold_regions(regions: &[mantis::fold::FoldRegion], path: &str, out: &mut impl Write) {
    let region_pairs: Vec<Vec<usize>> = regions.iter().map(|r| vec![r.start, r.end]).collect();
    let msg = serde_json::json!({
        "event": "action",
        "action": "set_fold_regions",
        "params": {
            "path": path,
            "regions": region_pairs
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
