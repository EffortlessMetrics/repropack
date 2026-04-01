# Implementation Plan: ReproPack v0.3 Beta

## Overview

Incremental implementation following the crate dependency chain: workspace deps → model → git → capture → replay → render → cli → schemas → property tests → integration tests → CLI smoke tests. Each task builds on the previous, with checkpoints after major layers. Property and integration tests are optional sub-tasks for faster MVP.

## Tasks

- [ ] 1. Add new workspace dependencies and update crate Cargo.toml files
  - Add `toml`, `reqwest` (blocking, rustls-tls), `regex`, and `zip` to `[workspace.dependencies]` in root `Cargo.toml`
  - Add `toml` as a dependency of `repropack-cli` and as `[dev-dependencies]` on `repropack-model`
  - Add `reqwest` and `zip` as dependencies of `repropack-cli`
  - Add `regex` as a dependency of `repropack-replay`
  - Add `proptest` as `[dev-dependencies]` on `repropack-render` and `repropack-cli` (already present on model, pack, git, capture, replay)
  - _Requirements: 5.1, 11.1, 14.1, 23.1_

- [ ] 2. Implement repropack-model new types and modified types
  - [ ] 2.1 Add new types: `RedactionEntry`, `RedactionAction`, `DoctorReport`, `DoctorReadiness`, `RedactionSummary`
    - Add `RedactionEntry` with `field_or_path`, `action`, `reason`
    - Add `RedactionAction` enum: `Replaced`, `Removed`, `Cleared`
    - Add `DoctorReport` with `readiness`, `omissions_by_kind`, `redacted_env_keys`, `tool_versions`, `missing_tools`, `has_redaction_report`, `redaction_summary`, `notes`
    - Add `DoctorReadiness` enum: `Ready`, `Degraded`, `Blocked`
    - Add `RedactionSummary` with `replaced_values`, `removed_files`
    - Derive `Serialize, Deserialize, PartialEq, Eq, Clone, Debug` on all types
    - _Requirements: 6.1, 6.5, 9.5, 10.1, 22.1_

  - [ ] 2.2 Add configuration types: `RepropackConfig`, `ProfileConfig`, `ResolvedConfig`
    - Add `RepropackConfig` with `env_allow`, `env_deny`, `max_file_size`, `max_packet_size`, `format`, `git_bundle`, `replay_policy`, `default_profile`, `profile` map
    - Add `ProfileConfig` with same keys minus `default_profile` and `profile`
    - Add `ResolvedConfig` with all fields non-optional (fully resolved)
    - Implement `RepropackConfig::resolve(profile_name)`, `from_toml()`, `to_toml()`
    - Use `#[serde(default, skip_serializing_if)]` for optional fields
    - _Requirements: 14.1, 14.2, 14.3, 15.1, 15.4, 16.1, 23.1_

  - [ ] 2.3 Add `extract_semver` helper function
    - Use regex to find first `MAJOR.MINOR.PATCH` pattern in a version string
    - Return `Option<String>` — `None` if no semver pattern found
    - _Requirements: 5.1, 5.2_

  - [ ] 2.4 Modify `ReplayReceipt` to add `env_excluded_keys` optional field
    - Add `env_excluded_keys: Option<Vec<String>>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`
    - _Requirements: 3.2, 20.1_

  - [ ] 2.5 Modify `PacketManifest` to add `redaction_report_path` optional field
    - Add `redaction_report_path: Option<String>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`
    - _Requirements: 9.6, 9.9, 20.2_

- [ ] 3. Checkpoint — model types
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Implement repropack-git changes
  - [ ] 4.1 Add path exclusion support to `capture_git_snapshot`
    - Modify signature to accept `exclude_paths: &[&str]`
    - Append pathspec exclusions (`-- ':!<path>'`) to `git status`, `git diff`, and `git ls-files` commands
    - Update all existing callers to pass empty exclusion list
    - _Requirements: 1.2, 19.1, 19.2_

  - [ ] 4.2 Add bundle creation fallback for root commits
    - When `git bundle create <bundle> <sha>` fails, retry with `git bundle create <bundle> --all`
    - On fallback success, record bundle path normally (no omission)
    - On both failures, record `Omission(kind: "bundle")` with reason
    - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [ ] 5. Implement repropack-capture changes
  - [ ] 5.1 Update default packet naming
    - When `--name` not provided: generate `repropack-<short-sha>-<YYYYMMDD-HHMMSS>` pattern
    - Slugify `--name` values to lowercase alphanumeric + hyphens
    - Append numeric suffix (`-1`, `-2`, ...) when output file already exists
    - _Requirements: 13.1, 13.2, 13.3, 13.4_

  - [ ] 5.2 Add `CaptureOptions::from_config` constructor
    - Map `ResolvedConfig` fields to `CaptureOptions` fields
    - _Requirements: 14.2, 16.2_

- [ ] 6. Implement repropack-replay changes
  - [ ] 6.1 Pass exclusion paths to `capture_git_snapshot` calls
    - Pass `&[".repropack-replay"]` for both pre-run and post-run snapshot captures
    - _Requirements: 1.1, 1.2, 1.3, 19.1, 19.2, 19.3_

  - [ ] 6.2 Implement env-excluded drift collapse
    - Add `collapse_env_excluded_drift` function returning a single summary `DriftItem` with subject `env_excluded_summary` and the full excluded key list
    - Replace per-variable `env_excluded:<KEY>` drift items with the single summary item
    - Store excluded key list in `receipt.env_excluded_keys`
    - _Requirements: 3.1, 3.2_

  - [ ] 6.3 Implement tool version normalization
    - Add `versions_match` function using `extract_semver` for comparison
    - Only emit `tool_version:<TOOL>` drift when normalized semver components differ
    - Fall back to exact string comparison when no semver pattern found
    - _Requirements: 5.1, 5.2, 5.3_

  - [ ] 6.4 Add `--verbose` flag support to `ReplayOptions`
    - Add `verbose: bool` to `ReplayOptions`
    - Wire through to control env-excluded output detail level
    - _Requirements: 3.3, 3.4, 18.1, 18.2_

- [ ] 7. Checkpoint — git, capture, and replay layers
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 8. Implement repropack-render new functions
  - [ ] 8.1 Add `render_doctor_text` and `render_doctor_json`
    - Render `DoctorReport` as human-readable text (default) or JSON
    - Include readiness, omission groups, redacted keys, tool versions, missing tools, redaction summary
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.6, 22.1, 22.2_

  - [ ] 8.2 Add `render_explain_output`
    - For each `DriftItem`, produce human-readable explanation based on subject pattern
    - Handle `stdout_digest`, `stderr_digest`, `output_digest:<PATH>`, `output_missing:<PATH>`, `capture_delta`, `tool_version:<TOOL>`, `env_excluded_summary`
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6_

  - [ ] 8.3 Add `render_gh_summary`
    - Render Markdown summary with packet name, commit SHA, command, exit code, fidelity, omission count
    - When receipt provided, include matched status and drift item count
    - Format for GitHub Actions `$GITHUB_STEP_SUMMARY`
    - _Requirements: 12.1, 12.2, 12.3_

  - [ ] 8.4 Update receipt renderer for env-excluded summary
    - Handle collapsed `env_excluded_summary` drift item
    - In verbose mode, expand full `env_excluded_keys` list
    - In summary mode, show only count
    - _Requirements: 3.3, 3.4, 18.2, 18.3_

- [ ] 9. Implement repropack-cli new subcommands and global flags
  - [ ] 9.1 Add global `--verbose` and `--profile` flags to CLI struct
    - Add `verbose: bool` (global, `-v`) and `profile: Option<String>` (global) to `Cli`
    - _Requirements: 18.1, 15.2_

  - [ ] 9.2 Implement config file discovery and parsing
    - Add `discover_config` function: walk from cwd up to git root looking for `.repropack.toml`
    - Parse config, resolve active profile, use as defaults (CLI flags override)
    - Warn on unrecognized keys, exit 1 on malformed TOML or missing profile
    - _Requirements: 14.1, 14.2, 14.4, 14.5, 14.6, 15.2, 15.3, 15.5_

  - [ ] 9.3 Implement `doctor` subcommand
    - Materialize packet, read manifest, group omissions, list redacted keys, probe tool versions
    - Check for `redaction-report.json`, assess readiness (ready/degraded/blocked)
    - Render as text (default) or JSON (`--json`)
    - Exit 0 for ready/degraded, 1 for blocked
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 22.1, 22.2_

  - [ ] 9.4 Implement `explain` subcommand
    - Read receipt, render human-readable explanation for each drift item
    - Exit 0 if status is `matched`, 1 otherwise
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7_

  - [ ] 9.5 Implement `shell` subcommand
    - Materialize packet, restore git state and inputs, set env baseline from manifest
    - Print banner (packet name, commit SHA, session info)
    - Launch `$SHELL` (fallback `/bin/sh`) with cwd = workdir
    - Do NOT execute predicate command; exit with shell's exit code
    - Support `--into <DIR>` flag
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7_

  - [ ] 9.6 Implement `scrub` subcommand and scrub engine
    - Materialize source packet into temp dir, read manifest
    - Replace all `allowed_vars` values with `[REDACTED]`
    - Replace `exec/stdout.log` and `exec/stderr.log` with placeholder, update digests
    - Remove files under `inputs/files/` and `outputs/files/`, record `Omission(kind: "scrubbed")`
    - Set `replay_fidelity` to `inspect_only`, set `redaction_report_path`
    - Generate `RedactionEntry` list, write `redaction-report.json`
    - Rewrite manifest, regenerate `summary.md`, `summary.html`, `integrity.json`
    - Pack into output `.rpk` (default: `<original>-scrubbed.rpk`)
    - Support `--output` flag
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6, 9.7, 9.8, 9.9, 9.10, 10.1, 10.2, 10.3, 10.4, 10.5_

  - [ ] 9.7 Implement `fetch gh` subcommand
    - Authenticate via `GH_TOKEN` / `GITHUB_TOKEN`
    - Support `--sha`, `--job`, `--repo`, `--output` flags
    - List artifacts for run, filter to `repropack-*`, download and extract zip
    - Infer repo from `git remote get-url origin` when `--repo` not provided
    - Exit 1 with available artifact names if no match found
    - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5, 11.6, 11.7_

  - [ ] 9.8 Implement `gh summarize` subcommand
    - Materialize packet, optionally read receipt, render via `render_gh_summary`
    - Write to stdout, `--output`, or append to `$GITHUB_STEP_SUMMARY`
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_

  - [ ] 9.9 Implement `config show` subcommand
    - Discover config, resolve with active profile, print as TOML with source comments
    - Print built-in defaults when no config file found
    - Support `--profile` flag
    - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5_

  - [ ] 9.10 Wire config-aware defaults into capture subcommand
    - Use `CaptureOptions::from_config` when config file is present
    - CLI flags override config values
    - _Requirements: 14.2, 14.4, 16.2, 16.3_

- [ ] 10. Checkpoint — render and CLI
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 11. Update JSON schemas for v0.3 fields
  - [ ] 11.1 Update `schema/manifest-v1.schema.json`
    - Add optional `redaction_report_path` (string or null) to root properties
    - _Requirements: 20.2, 20.3_

  - [ ] 11.2 Update `schema/receipt-v1.schema.json`
    - Add optional `env_excluded_keys` (array of strings) to root properties
    - _Requirements: 20.1, 20.3_

- [ ] 12. Property tests for repropack-model
  - [ ]* 12.1 Write property test for config TOML round-trip (Property 16)
    - **Property 16: Config TOML round-trip**
    - Create `arb_repropack_config()` generator producing valid configs with 0–3 profiles
    - Verify serialize to TOML → parse produces equal value
    - **Validates: Requirements 17.5, 23.1, 23.3, 27.1**

  - [ ]* 12.2 Write property test for profile merge correctness (Property 17)
    - **Property 17: Profile merge overrides top-level defaults correctly**
    - Generate config with named profile, resolve, verify profile values override top-level and absent keys fall back
    - **Validates: Requirements 14.2, 14.4, 15.2, 15.4, 15.5, 16.1, 16.2, 16.3, 27.2**

  - [ ]* 12.3 Write property test for manifest serde round-trip with v0.3 fields (Property 18)
    - **Property 18: Manifest serde round-trip with v0.3 fields**
    - Extend `arb_packet_manifest()` to include `redaction_report_path`
    - Verify serialize → deserialize produces equal value and JSON validates against schema
    - **Validates: Requirements 20.4**

  - [ ]* 12.4 Write property test for receipt serde round-trip with v0.3 fields (Property 19)
    - **Property 19: Receipt serde round-trip with v0.3 fields**
    - Extend `arb_replay_receipt()` to include `env_excluded_keys`
    - Verify serialize → deserialize produces equal value and JSON validates against schema
    - **Validates: Requirements 20.5**

- [ ] 13. Property tests for repropack-git
  - [ ]* 13.1 Write property test for git snapshot path exclusion (Property 1)
    - **Property 1: Git snapshot path exclusion filters correctly**
    - Generate arbitrary path lists and exclusion prefixes, verify no excluded paths appear in output
    - Place in `tests/snapshot_exclusion_props.rs`
    - **Validates: Requirements 1.2, 19.1, 19.2**

- [ ] 14. Property tests for repropack-replay
  - [ ]* 14.1 Write property test for env-excluded drift collapse (Property 2)
    - **Property 2: Env-excluded drift collapse produces correct summary and key list**
    - Generate arbitrary host and final env maps, verify single summary drift item with correct count and key list
    - Place in `tests/env_props.rs`
    - **Validates: Requirements 3.1, 3.2**

  - [ ]* 14.2 Write property test for tool version normalization (Property 3)
    - **Property 3: Tool version normalization determines drift correctly**
    - Generate version string pairs with embedded semver, verify drift emitted iff semver components differ
    - Place in `tests/version_props.rs`
    - **Validates: Requirements 5.1, 5.2, 5.3**

- [ ] 15. Property tests for repropack-render
  - [ ]* 15.1 Write property test for explain output coverage (Property 6)
    - **Property 6: Explain output covers all drift items**
    - Generate receipts with arbitrary drift items, verify output contains subject of each
    - Place in `tests/explain_props.rs`
    - **Validates: Requirements 7.1**

  - [ ]* 15.2 Write property test for GH summary content (Property 13)
    - **Property 13: GitHub summary contains required manifest fields**
    - Generate manifests and optional receipts, verify output contains packet name/ID, commit SHA, command, exit code, fidelity, omission count
    - Place in `tests/gh_summary_props.rs`
    - **Validates: Requirements 12.1, 12.3**

- [ ] 16. Property tests for repropack-capture
  - [ ]* 16.1 Write property test for slug function validity (Property 14)
    - **Property 14: Slug function produces valid packet names**
    - Generate arbitrary strings, verify slug output is lowercase alphanumeric + hyphens, no leading/trailing/consecutive hyphens
    - Place in `tests/naming_props.rs`
    - **Validates: Requirements 13.2**

  - [ ]* 16.2 Write property test for default packet name pattern (Property 15)
    - **Property 15: Default packet name follows pattern**
    - Generate manifests with git commit SHA and no explicit name, verify output matches `repropack-<8-char>-<YYYYMMDD-HHMMSS>.rpk`
    - Place in `tests/naming_props.rs`
    - **Validates: Requirements 13.1, 13.3**

- [ ] 17. Property tests for repropack-cli (scrub and doctor)
  - [ ]* 17.1 Write property test for scrub replaces allowed_vars (Property 7)
    - **Property 7: Scrub replaces all allowed_vars values with [REDACTED]**
    - Generate manifests with non-empty `allowed_vars`, scrub, verify all values are `[REDACTED]` and keys unchanged
    - Place in `tests/scrub_props.rs`
    - **Validates: Requirements 9.1, 28.2**

  - [ ]* 17.2 Write property test for scrub preserves structural metadata (Property 8)
    - **Property 8: Scrub preserves structural metadata**
    - Verify command record, execution record, and git state metadata are preserved after scrub
    - Place in `tests/scrub_props.rs`
    - **Validates: Requirements 9.4**

  - [ ]* 17.3 Write property test for scrub schema + integrity validity (Property 9)
    - **Property 9: Scrub produces a schema-valid packet with consistent integrity**
    - Verify scrubbed manifest passes schema validation and integrity.json is consistent
    - Place in `tests/scrub_props.rs`
    - **Validates: Requirements 9.7, 9.10, 21.1, 21.3, 21.4, 28.1, 28.3**

  - [ ]* 17.4 Write property test for scrub sets inspect_only (Property 10)
    - **Property 10: Scrub sets replay_fidelity to inspect_only**
    - Place in `tests/scrub_props.rs`
    - **Validates: Requirements 9.8, 28.4**

  - [ ]* 17.5 Write property test for redaction report completeness (Property 11)
    - **Property 11: Redaction report is complete and correct**
    - Verify one `replaced` entry per allowed_var key, one `removed` per input/output file, `cleared` entries for stdout/stderr logs
    - Place in `tests/scrub_props.rs`
    - **Validates: Requirements 10.1, 10.2, 10.3, 10.4, 10.5**

  - [ ]* 17.6 Write property test for scrub removes input/output files (Property 12)
    - **Property 12: Scrub removes input and output file contents**
    - Verify `inputs/files/` and `outputs/files/` are empty, manifest arrays updated, `Omission(kind: "scrubbed")` recorded
    - Place in `tests/scrub_props.rs`
    - **Validates: Requirements 9.3**

  - [ ]* 17.7 Write property test for scrub updates log digests (Property 21)
    - **Property 21: Scrub updates stdout/stderr digests to match redacted content**
    - Verify `stdout_sha256` and `stderr_sha256` equal SHA-256 of placeholder content
    - Place in `tests/scrub_props.rs`
    - **Validates: Requirements 9.2**

  - [ ]* 17.8 Write property test for doctor on scrubbed packets (Property 20)
    - **Property 20: Doctor on scrubbed packets reports blocked with redaction summary**
    - Verify readiness is `blocked`, `has_redaction_report` is true, redaction summary counts are correct
    - Place in `tests/scrub_props.rs`
    - **Validates: Requirements 21.2, 22.1, 22.2**

  - [ ]* 17.9 Write property test for doctor readiness assessment (Property 4)
    - **Property 4: Doctor readiness assessment follows rules**
    - Generate manifests with varying bundle/omission/policy states, verify readiness classification
    - Place in `tests/doctor_props.rs`
    - **Validates: Requirements 6.5**

  - [ ]* 17.10 Write property test for doctor omission grouping (Property 5)
    - **Property 5: Doctor report groups omissions by kind**
    - Generate manifests with omissions, verify grouping and union equals original list
    - Place in `tests/doctor_props.rs`
    - **Validates: Requirements 6.2, 6.3**

- [ ] 18. Checkpoint — schemas and property tests
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 19. Integration tests — scenario suite
  - [ ]* 19.1 Root-commit repo capture scenario
    - Create temp Git repo with only a root commit, capture, assert `bundle_path` is present in manifest
    - _Requirements: 2.4, 25.1_

  - [ ]* 19.2 Clean deterministic capture + replay match scenario
    - Create temp repo, commit deterministic passing script, capture, replay, assert `receipt.matched == true` with zero warning/error drift
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 25.2_

  - [ ]* 19.3 Replay support dir exclusion scenario
    - Replay a packet, assert no `.repropack-replay/` paths appear in capture delta drift
    - _Requirements: 1.1, 1.3, 25.3_

  - [ ]* 19.4 Env-excluded drift collapse scenario
    - Replay with default settings, assert at most one `env_excluded_summary` drift item instead of per-variable items
    - _Requirements: 3.1, 25.4_

  - [ ]* 19.5 Scrub workflow: capture with env vars + outputs, scrub, assert redaction
    - Capture packet with env vars and output files, scrub with `--public`, assert all `allowed_vars` values are `[REDACTED]`
    - _Requirements: 26.1_

  - [ ]* 19.6 Scrub workflow: scrub then inspect succeeds
    - Scrub a packet, run `repropack inspect` on scrubbed packet, assert success
    - _Requirements: 26.2_

  - [ ]* 19.7 Scrub workflow: assert redaction report entries
    - Scrub a packet, read `redaction-report.json`, assert entries for each redacted field and removed file
    - _Requirements: 26.3_

  - [ ]* 19.8 Scrub workflow: assert replay_fidelity is inspect_only
    - Scrub a packet, read scrubbed manifest, assert `replay_fidelity` is `inspect_only`
    - _Requirements: 26.4_

- [ ] 20. CLI smoke tests for new subcommands
  - [ ]* 20.1 `repropack doctor --help` exits 0
    - _Requirements: 24.1_
  - [ ]* 20.2 `repropack explain --help` exits 0
    - _Requirements: 24.2_
  - [ ]* 20.3 `repropack shell --help` exits 0
    - _Requirements: 24.3_
  - [ ]* 20.4 `repropack scrub --help` exits 0
    - _Requirements: 24.4_
  - [ ]* 20.5 `repropack fetch --help` exits 0
    - _Requirements: 24.5_
  - [ ]* 20.6 `repropack gh --help` exits 0
    - _Requirements: 24.6_
  - [ ]* 20.7 `repropack config --help` exits 0
    - _Requirements: 24.7_

- [ ] 21. Final checkpoint — full CI
  - Ensure all tests pass via `cargo xtask ci-full`, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Dependency order: model (task 2) → git (task 4) → capture/replay (tasks 5–6) → render (task 8) → cli (task 9) → schemas (task 11) → tests (tasks 12–20)
- Each task references specific requirements for traceability
- Property tests reference their design property number
- Checkpoints at tasks 3, 7, 10, 18, and 21 ensure incremental validation
- The scrub engine lives in `repropack-cli` (task 9.6) since it orchestrates across pack, model, and render
- `reqwest` and `zip` are only used by `repropack-cli` for the `fetch gh` subcommand
- All new manifest/receipt fields use `Option` with serde defaults for backward compatibility with v0.2 packets
