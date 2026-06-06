use crate::models::CrateReport;
use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::fs;

pub async fn check_crate(cargo_toml_path: &Path) -> CrateReport {
    let dir = cargo_toml_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let mut report = CrateReport::new(name.clone(), cargo_toml_path.to_path_buf());

    // Read Cargo.toml
    let content = match fs::read_to_string(cargo_toml_path).await {
        Ok(c) => c,
        Err(e) => {
            report.add("cargo-toml-readable", false, Some(e.to_string()));
            return report;
        }
    };

    let manifest: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            report.add("cargo-toml-valid", false, Some(e.to_string()));
            return report;
        }
    };

    let package = manifest.get("package");

    // Check 1: Has package name
    let pkg_name = package
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(&name);
    report.add("has-name", true, None);

    // Update report name from manifest
    report.name = pkg_name.to_string();

    // Check 2: Unique name on crates.io
    match check_crates_io_unique(pkg_name).await {
        Ok(is_unique) => {
            report.add(
                "unique-name",
                is_unique,
                if is_unique {
                    None
                } else {
                    Some(format!("{} already exists on crates.io", pkg_name))
                },
            );
        }
        Err(e) => {
            report.add("unique-name", false, Some(format!("Check failed: {}", e)));
        }
    }

    // Check 3: Has description
    let has_desc = package
        .and_then(|p| p.get("description"))
        .and_then(|d| d.as_str())
        .map(|d| !d.is_empty())
        .unwrap_or(false);
    report.add(
        "has-description",
        has_desc,
        if has_desc {
            None
        } else {
            Some("Missing [package] description".into())
        },
    );

    // Check 4: Has license
    let has_license = package
        .and_then(|p| p.get("license"))
        .and_then(|l| l.as_str())
        .map(|l| !l.is_empty())
        .unwrap_or(false);
    report.add(
        "has-license",
        has_license,
        if has_license {
            None
        } else {
            Some("Missing [package] license".into())
        },
    );

    // Check 5: Has repository
    let has_repo = package
        .and_then(|p| p.get("repository"))
        .and_then(|r| r.as_str())
        .map(|r| !r.is_empty())
        .unwrap_or(false);
    report.add(
        "has-repository",
        has_repo,
        if has_repo {
            None
        } else {
            Some("Missing [package] repository".into())
        },
    );

    // Check 6: Has src/lib.rs or src/main.rs (not empty)
    check_source_files(&dir, &mut report).await;

    // Check 7: Has at least 1 test
    check_has_tests(&dir, &content, &mut report).await;

    // Check 8: No path-only dependencies
    check_path_dependencies(&manifest, &mut report);

    // Check 9: cargo check passes
    check_cargo_check(&dir, &mut report).await;

    report
}

async fn check_crates_io_unique(name: &str) -> Result<bool> {
    let client = reqwest::Client::builder()
        .user_agent("crates-publish-check/0.1.0")
        .build()?;

    let url = format!("https://crates.io/api/v1/crates/{}", name);
    let resp = client.get(&url).send().await;

    match resp {
        Ok(r) => {
            // If we get 200, the crate exists
            Ok(r.status() != reqwest::StatusCode::OK)
        }
        Err(e) => {
            // Network error - be conservative, mark as failed
            Err(anyhow::anyhow!("HTTP error: {}", e))
        }
    }
}

async fn check_source_files(dir: &Path, report: &mut CrateReport) {
    let lib_rs = dir.join("src/lib.rs");
    let main_rs = dir.join("src/main.rs");

    let (lib_exists, main_exists) = (lib_rs.exists(), main_rs.exists());

    if !lib_exists && !main_exists {
        report.add(
            "has-source",
            false,
            Some("No src/lib.rs or src/main.rs found".into()),
        );
        return;
    }

    // Check that the source file is not empty
    async fn check_empty(path: &Path) -> bool {
        match fs::read_to_string(path).await {
            Ok(content) => {
                let trimmed = content.trim();
                trimmed
                    .lines()
                    .any(|line| !line.is_empty() && !line.starts_with("//"))
            }
            Err(_) => false,
        }
    }

    let has_content = if lib_exists {
        check_empty(&lib_rs).await
    } else {
        check_empty(&main_rs).await
    };

    report.add(
        "has-source",
        has_content,
        if has_content {
            None
        } else {
            Some("Source file is empty or only comments".into())
        },
    );
}

async fn check_has_tests(dir: &Path, cargo_content: &str, report: &mut CrateReport) {
    // Check for #[test] in source files, tests/ directory, or [[test]] in Cargo.toml
    let has_test_attr = async {
        // Check src/lib.rs, src/main.rs, and src/**/*.rs
        let src_dir = dir.join("src");
        if src_dir.exists() {
            if let Ok(mut entries) = fs::read_dir(&src_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(content) = fs::read_to_string(entry.path()).await {
                        if content.contains("#[test]") {
                            return true;
                        }
                    }
                }
            }
        }
        false
    };

    let has_tests_dir = dir.join("tests").exists();
    let has_test_section = cargo_content.contains("#[test]")
        || cargo_content.contains("[[test]]")
        || cargo_content.contains("[[bench]]");

    let found = has_test_attr.await || has_tests_dir || has_test_section;

    report.add(
        "has-tests",
        found,
        if found {
            None
        } else {
            Some("No tests found (no #[test], tests/ dir, or [[test]])".into())
        },
    );
}

fn check_path_dependencies(manifest: &toml::Value, report: &mut CrateReport) {
    let mut path_deps = Vec::new();

    for section in &["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = manifest.get(section).and_then(|d| d.as_table()) {
            for (name, value) in deps {
                match value {
                    toml::Value::String(_) => {
                        // version string only, OK
                    }
                    toml::Value::Table(t) => {
                        if t.contains_key("path") && !t.contains_key("version") {
                            path_deps.push(name.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    report.add(
        "no-path-only-deps",
        path_deps.is_empty(),
        if path_deps.is_empty() {
            None
        } else {
            Some(format!(
                "Path-only dependencies: {}",
                path_deps.join(", ")
            ))
        },
    );
}

async fn check_cargo_check(dir: &Path, report: &mut CrateReport) {
    let output = tokio::process::Command::new("cargo")
        .arg("check")
        .current_dir(dir)
        .output()
        .await;

    match output {
        Ok(out) => {
            let success = out.status.success();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            report.add(
                "cargo-check",
                success,
                if success {
                    None
                } else {
                    let first_err = stderr
                        .lines()
                        .find(|l| l.contains("error"))
                        .unwrap_or("Unknown error");
                    Some(first_err.to_string())
                },
            );
        }
        Err(e) => {
            report.add(
                "cargo-check",
                false,
                Some(format!("Failed to run cargo check: {}", e)),
            );
        }
    }
}
