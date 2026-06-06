pub mod checks;
pub mod models;
mod report;

use anyhow::Result;
use clap::Parser;
use console::style;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use tokio::fs;

use checks::check_crate;
use models::CrateReport;
use report::print_report;

/// Check which Rust crates in a directory are ready for crates.io publishing.
#[derive(Parser, Debug)]
#[command(name = "crates-publish-check", version, about)]
struct Args {
    /// Directory to scan for crates
    #[arg(default_value = ".")]
    directory: PathBuf,

    /// Number of crates to process concurrently
    #[arg(long, default_value = "4")]
    batch: usize,

    /// Run `cargo publish --dry-run` on ready crates
    #[arg(long)]
    publish: bool,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Only show crates that are ready
    #[arg(long)]
    ready_only: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if !args.directory.exists() {
        anyhow::bail!("Directory does not exist: {}", args.directory.display());
    }

    // Discover all Cargo.toml files
    let crates = discover_crates(&args.directory).await?;
    if crates.is_empty() {
        println!("{}", style("No crates found.").yellow());
        return Ok(());
    }

    println!(
        "{}",
        style(format!("Found {} crate(s) to check", crates.len())).cyan()
    );

    // Process crates in batches
    let pb = ProgressBar::new(crates.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")?
            .progress_chars("#>-"),
    );

    let results: Vec<CrateReport> = stream::iter(crates)
        .map(|path| {
            let pb = pb.clone();
            async move {
                let name = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                pb.set_message(name.clone());
                let report = check_crate(&path).await;
                pb.inc(1);
                report
            }
        })
        .buffer_unordered(args.batch)
        .collect()
        .await;

    pb.finish_with_message("done");

    // Separate into ready and unready
    let mut ready: Vec<&CrateReport> = Vec::new();
    let mut unready: Vec<&CrateReport> = Vec::new();

    for r in &results {
        if r.is_ready() {
            ready.push(r);
        } else {
            unready.push(r);
        }
    }

    // Handle --publish flag
    let publish_results = if args.publish && !ready.is_empty() {
        Some(run_dry_publish(&ready).await?)
    } else {
        None
    };

    // Output
    if args.json {
        print_json(&results, &publish_results, args.ready_only)?;
    } else {
        print_report(&results, &publish_results, args.ready_only)?;
    }

    Ok(())
}

async fn discover_crates(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut crates = Vec::new();
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let cargo_toml = path.join("Cargo.toml");
            if cargo_toml.exists() {
                // Verify it's a valid Cargo.toml
                let content = fs::read_to_string(&cargo_toml).await?;
                if content.contains("[package]") {
                    crates.push(cargo_toml);
                }
            }
        }
    }

    // Check if the directory itself is a crate
    let root_cargo = dir.join("Cargo.toml");
    if root_cargo.exists() {
        let content = fs::read_to_string(&root_cargo).await?;
        if content.contains("[package]") {
            // Only add if not already included (subdir case)
            if !crates.iter().any(|p| p == &root_cargo) {
                crates.push(root_cargo);
            }
        }
    }

    crates.sort();
    Ok(crates)
}

async fn run_dry_publish(ready: &[&CrateReport]) -> Result<Vec<(String, bool, String)>> {
    let mut results = Vec::new();

    println!(
        "\n{}",
        style(format!(
            "Running cargo publish --dry-run on {} crate(s)...",
            ready.len()
        ))
        .cyan()
    );

    for report in ready {
        let dir = report.path.parent().unwrap_or(std::path::Path::new("."));
        let output = tokio::process::Command::new("cargo")
            .args(["publish", "--dry-run"])
            .current_dir(dir)
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let success = out.status.success();
                let combined = if success { stdout } else { stderr };
                results.push((report.name.clone(), success, combined));
            }
            Err(e) => {
                results.push((report.name.clone(), false, e.to_string()));
            }
        }
    }

    Ok(results)
}

fn print_json(
    results: &[CrateReport],
    publish_results: &Option<Vec<(String, bool, String)>>,
    ready_only: bool,
) -> Result<()> {
    use serde_json::json;

    let filtered: Vec<&CrateReport> = if ready_only {
        results.iter().filter(|r| r.is_ready()).collect()
    } else {
        results.iter().collect()
    };

    let ready: Vec<&CrateReport> = filtered.iter().filter(|r| r.is_ready()).copied().collect();
    let unready: Vec<&CrateReport> = filtered
        .iter()
        .filter(|r| !r.is_ready())
        .copied()
        .collect();

    let mut obj = json!({
        "ready": ready,
        "unready": unready,
        "total": results.len(),
        "ready_count": ready.len(),
    });

    if let Some(pub_results) = publish_results {
        obj["publish_dry_run"] = json!(pub_results
            .iter()
            .map(|(name, success, output)| {
                json!({
                    "crate": name,
                    "success": success,
                    "output": output,
                })
            })
            .collect::<Vec<_>>());
    }

    println!("{}", serde_json::to_string_pretty(&obj)?);
    Ok(())
}
