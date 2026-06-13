# Crates Publish Check

**Crates Publish Check** is a Rust CLI tool that scans a directory of Rust crates and determines which are ready for publishing to crates.io — checking metadata completeness, license compliance, documentation, and optionally running `cargo publish --dry-run`.

## Why It Matters

Publishing a crate to crates.io requires more than working code: the manifest needs valid metadata (description, license, repository, keywords), the crate must compile without errors, documentation should be present, and the package must not contain secrets or large binary files. When managing a fleet of 40+ crates, manually checking each one before publication is error-prone and tedious. This tool automates the pre-publication checklist: it parses each `Cargo.toml`, validates the metadata against crates.io requirements, checks for common issues (missing README, missing license file, version conflicts), and optionally runs the actual dry-run publish to catch issues that static analysis misses.

## How It Works

**Discovery phase:**
```
discover(directory):
  for each subdirectory in directory:
    if subdirectory/Cargo.toml exists and contains "[package]":
      add to crate list
  also check directory itself (root-level crate)
```

**Check pipeline (per crate):**
Each crate is processed through a series of checks, executed concurrently with configurable batch size:

1. **Metadata check:** Verify `name`, `version`, `description`, `license`, `repository` fields are present and non-empty in `[package]`.

2. **Uniqueness check:** Query crates.io API to see if the name is already taken (requires `reqwest` async HTTP).

3. **Compile check:** Run `cargo check` to verify the crate compiles without errors.

4. **Documentation check:** Verify `README.md` exists and is non-trivial (> 1KB).

5. **License check:** Verify `LICENSE` file exists and matches the declared SPDX license.

6. **Dry-run publish (optional):** Run `cargo publish --dry-run` which performs the actual packaging and validation without uploading.

**Result aggregation:** Each crate is classified as Ready or Not Ready, with a list of issues for unready crates.

**Concurrency:** Crates are processed via `tokio::stream::buffer_unordered(batch_size)`, defaulting to 4 concurrent checks to avoid overwhelming the local cargo registry.

## Quick Start

```bash
# Check all crates in a directory
cargo run -- /path/to/fleet

# Show only ready crates
cargo run -- --ready-only /path/to/fleet

# Output as JSON
cargo run -- --json /path/to/fleet

# Run dry-run publish on ready crates
cargo run -- --publish /path/to/fleet
```

## API

| Module | Description |
|--------|-------------|
| `checks` | Per-crate validation logic |
| `models::CrateReport` | Full report for one crate |
| `report` | Formatted output (text/JSON) |
| `discover_crates` | Find all Cargo.toml in a directory tree |

## Architecture Notes

Crates Publish Check provides the **publication readiness gate** for the SuperInstance fleet. Within γ + η = C, it ensures that conservation-law implementations are properly documented and packaged before fleet-wide deployment — preventing γ-layer computation crates from shipping without the metadata that η-layer intelligence depends on for dependency resolution.

See [ARCHITECTURE.md](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md).

**Check depth:** The tool performs static analysis by default (fast, O(n) per crate). The `--publish` flag adds dynamic analysis (`cargo publish --dry-run`), which invokes the actual Cargo packaging pipeline. Dynamic analysis catches issues that static checks miss: path resolution errors, feature flag incompatibilities, and build script failures. The trade-off is runtime: static checks complete in <1s per crate, while dry-run publish can take 30–60s per crate (full compile + package).

**Concurrent processing:** Crates are processed via `buffer_unordered(4)`, meaning up to 4 crates are checked simultaneously. This achieves near-linear speedup on multi-core machines while avoiding over-subscription of the cargo registry lock (which serializes package builds locally).

## References

1. Cargo Book (2024). "Publishing on crates.io." *The Cargo Book*, Chapter 14.
2. SPDX (2023). *SPDX License List*. Linux Foundation.

## License

MIT
