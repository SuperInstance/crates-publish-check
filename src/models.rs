use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct CrateReport {
    pub name: String,
    pub path: PathBuf,
    pub checks: Vec<CheckResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub check: String,
    pub passed: bool,
    pub message: Option<String>,
}

impl CrateReport {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            checks: Vec::new(),
        }
    }

    pub fn add(&mut self, check: &str, passed: bool, message: Option<String>) {
        self.checks.push(CheckResult {
            check: check.to_string(),
            passed,
            message,
        });
    }

    pub fn is_ready(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }

    pub fn failed_checks(&self) -> Vec<&CheckResult> {
        self.checks.iter().filter(|c| !c.passed).collect()
    }
}
