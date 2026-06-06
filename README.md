# crates-publish-check

CLI tool that checks which Rust crates in a directory are ready for crates.io publishing.

## Features

- **Name Availability** — Check if crate name is already taken on crates.io
- **Metadata Validation** — Verify description, license, repository in Cargo.toml
- **Source Check** — Ensure src/lib.rs or src/main.rs exists and is non-empty
- **Test Check** — Verify at least one `#[test]` exists
- **Dependency Check** — Flag path-only dependencies that won't resolve on crates.io
- **Batch Processing** — Scan entire fleets of crates at once

## Installation

```bash
cargo install --git https://github.com/SuperInstance/crates-publish-check
```

## Usage

```bash
# Check all crates in a directory
cargo-publish-check scan ~/repos

# Check a single crate
cargo-publish-check check ~/repos/my-crate

# Dry-run publish on ready crates
cargo-publish-check scan ~/repos --publish
```

## Checks Performed

| Check | Description |
|-------|-------------|
| `unique-name` | Name not already on crates.io |
| `has-metadata` | description, license, repository in Cargo.toml |
| `has-source` | src/lib.rs or src/main.rs exists and is non-empty |
| `has-tests` | At least one `#[test]` attribute found |
| `no-path-deps` | No path-only dependencies |
| `compiles` | `cargo check` passes |

## Output

```json
{
  "ready": [
    {"name": "my-crate", "version": "0.1.0", "checks": 6}
  ],
  "unready": [
    {"name": "wip-crate", "issues": ["no-tests", "no-license"]}
  ]
}
```

## Testing

```bash
cargo test
```

## License

MIT
