use crate::models::CrateReport;
use anyhow::Result;
use console::style;

pub fn print_report(
    results: &[CrateReport],
    publish_results: &Option<Vec<(String, bool, String)>>,
    ready_only: bool,
) -> Result<()> {
    let ready: Vec<&CrateReport> = results.iter().filter(|r| r.is_ready()).collect();
    let unready: Vec<&CrateReport> = results.iter().filter(|r| !r.is_ready()).collect();

    println!("\n{}", style("═══ Publish Readiness Report ═══").bold().cyan());
    println!(
        "  Total: {} | {} | {}",
        style(results.len()).bold(),
        style(format!("✓ {} ready", ready.len())).green(),
        style(format!("✗ {} unready", unready.len())).red()
    );

    if !ready.is_empty() && !ready_only {
        println!("\n{}", style("Ready for publishing:").green().bold());
        for r in &ready {
            println!("  {} {}", style("✓").green(), style(&r.name).bold());
        }
    }

    if !unready.is_empty() && !ready_only {
        println!("\n{}", style("Not ready:").red().bold());
        for r in &unready {
            println!("  {} {}", style("✗").red(), style(&r.name).bold());
            for failed in r.failed_checks() {
                let msg = failed
                    .message
                    .as_ref()
                    .map(|m| format!(": {}", m))
                    .unwrap_or_default();
                println!("    {} {}{}", style("•").yellow(), failed.check, msg);
            }
        }
    }

    if ready_only && !ready.is_empty() {
        println!("\n{}", style("Ready for publishing:").green().bold());
        for r in &ready {
            println!("  {} {}", style("✓").green(), style(&r.name).bold());
        }
    }

    if let Some(pub_results) = publish_results {
        println!("\n{}", style("Publish dry-run results:").cyan().bold());
        for (name, success, output) in pub_results {
            let status = if *success {
                style("✓ PASSED").green()
            } else {
                style("✗ FAILED").red()
            };
            println!("  {} {} — {}", name, status, output.lines().next().unwrap_or(""));
        }
    }

    println!();
    Ok(())
}
