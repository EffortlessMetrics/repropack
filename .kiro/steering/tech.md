# Tech Stack

## Language and toolchain

- Rust, edition 2021
- Stable channel with `clippy` and `rustfmt` components (pinned via `rust-toolchain.toml`)
- Cargo workspace with resolver v2

## Key dependencies

| Crate | Purpose |
|---|---|
| `serde` / `serde_json` | Serialization for manifest, receipt, and all JSON artifacts |
| `clap` (derive) | CLI argument parsing |
| `anyhow` | Error handling throughout application crates |
| `tar` / `flate2` | `.rpk` archive creation and extraction (gzip-compressed tar) |
| `sha2` | SHA-256 digests for packet file integrity |
| `uuid` (v4) | Packet ID generation |
| `time` (RFC 3339) | Timestamps in manifests and receipts |
| `glob` | File pattern matching for `--include` and `--output` globs |
| `walkdir` | Recursive directory traversal |
| `tempfile` | Staging directories during capture and materialization |

## Conventions

- All JSON output uses `serde_json::to_vec_pretty` for human-readable formatting.
- Enums use `#[serde(rename_all = "snake_case")]`.
- Ordered collections use `BTreeMap` for deterministic serialization.
- Archive entries are sorted by path with normalized timestamps (mtime=0).
- Error handling: `anyhow::Result` in application crates, `std::io::Result` in model.
- Git operations shell out to the `git` CLI (no libgit2 dependency).

## Commands

```bash
# Fast CI check (format + clippy)
cargo xtask ci-fast

# Full CI (fast + tests)
cargo xtask ci-full

# Smoke test (--help + emit)
cargo xtask smoke

# Regenerate scenario atlas
cargo xtask scenario-index

# Check required docs exist
cargo xtask docs-check

# Full release readiness check
cargo xtask release-check

# Run mutation tests
cargo xtask mutants
```

The `just` command runner wraps these same xtask commands for convenience.

Cargo alias: `cargo xtask` is configured in `.cargo/config.toml` as `run -p xtask --`.

## Schemas

JSON schemas for the manifest and receipt live in `schema/`:
- `schema/manifest-v1.schema.json`
- `schema/receipt-v1.schema.json`

Sample files in `examples/` match these schemas.
