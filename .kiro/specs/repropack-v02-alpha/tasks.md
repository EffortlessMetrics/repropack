# Implementation Plan: ReproPack v0.2 Alpha

## Overview

Incremental implementation following the crate dependency chain: model (no deps) → pack/git (depend on model) → capture/replay (depend on model + edge crates) → render/cli (depend on orchestration crates) → integration and smoke tests. Each task builds on the previous, and property tests are placed close to the code they validate.

## Tasks

- [x] 1. Add workspace dependencies and update crate Cargo.toml files
  - Add `jsonschema` and `proptest` to `[workspace.dependencies]` in root `Cargo.toml`
  - Add `jsonschema` as a dependency of `repropack-model`
  - Add `proptest` as `[dev-dependencies]` on `repropack-model`, `repropack-pack`, `repropack-git`, `repropack-capture`, `repropack-replay`
  - _Requirements: 9.1, 14.1, 15.1_

- [ ] 2. Implement repropack-model new types and modified types
  - [x] 2.1 Add new structs: `GitSnapshot`, `CaptureDelta`, `EnvClassification`, `IntegrityEntry`, `SizeCaps`
    - Add `GitSnapshot` with `commit_sha`, `is_dirty`, `changed_paths`, `untracked_paths`, `worktree_patch_path`
    - Add `CaptureDelta` with `newly_dirty_paths`, `newly_modified_paths`, `newly_untracked_paths`
    - Add `EnvClassification` with `restored`, `overridden`, `inherited` arrays
    - Add `IntegrityEntry` with `relative_path`, `sha256`, `size_bytes`
    - Add `SizeCaps` with `max_file_bytes` (default 50 MiB), `max_packet_bytes` (default 500 MiB)
    - Derive `PartialEq, Eq` on types that need property testing
    - _Requirements: 1.2, 2.2, 3.2, 5.1, 10.1, 11.1_

  - [x] 2.2 Modify `GitState` to add `git_pre`, `git_post`, `capture_delta` optional fields
    - Add `git_pre: Option<GitSnapshot>`, `git_post: Option<GitSnapshot>`, `capture_delta: Option<CaptureDelta>`
    - Use `#[serde(default, skip_serializing_if = "Option::is_none")]` for backward compatibility
    - _Requirements: 1.2, 2.2, 3.2_

  - [x] 2.3 Modify `ExecutionRecord` to add `stdout_sha256` and `stderr_sha256` optional fields
    - Add `stdout_sha256: Option<String>`, `stderr_sha256: Option<String>` with serde defaults
    - _Requirements: 6.1_

  - [x] 2.4 Modify `ReplayReceipt` to add `env_classification` and `matched_outputs` optional fields
    - Add `env_classification: Option<EnvClassification>`, `matched_outputs: Option<bool>` with serde defaults
    - _Requirements: 5.1, 7.4, 8.2_

- [ ] 3. Implement schema validation module in repropack-model
  - [x] 3.1 Create `validate` module with `validate_manifest` and `validate_receipt` functions
    - Embed JSON Schema files via `include_str!`
    - Compile schemas using `jsonschema::JSONSchema`
    - Return `ValidationError` with path and message on failure
    - _Requirements: 9.1, 9.2, 9.3, 9.4_

  - [x] 3.2 Integrate schema validation into `PacketManifest::read_from_path` and `ReplayReceipt::read_from_path`
    - Parse JSON to `serde_json::Value` first, validate against schema, then deserialize
    - Return error with validation failure path on schema mismatch
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 17.2, 17.3_

- [ ] 4. Update JSON Schema files for v0.2 fields
  - [x] 4.1 Update `schema/manifest-v1.schema.json`
    - Add `gitSnapshot` and `captureDelta` definitions to `$defs`
    - Add optional `git_pre`, `git_post`, `capture_delta` properties to `gitState`
    - Add optional `stdout_sha256`, `stderr_sha256` to `execution`
    - _Requirements: 12.1, 12.3_

  - [x] 4.2 Update `schema/receipt-v1.schema.json`
    - Add `envClassification` definition to `$defs`
    - Add optional `env_classification`, `matched_outputs` to root properties
    - _Requirements: 12.2, 12.3_

- [x] 5. Checkpoint — model and schemas
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 6. Implement repropack-pack integrity envelope and sha256_bytes
  - [x] 6.1 Add `sha256_bytes` helper function
    - Compute SHA-256 of a `&[u8]` slice, return hex string
    - _Requirements: 6.1_

  - [x] 6.2 Add `verify_integrity` function
    - Read `integrity.json` from packet root, parse as `Vec<IntegrityEntry>`
    - For each entry, compute SHA-256 of the actual file and compare
    - Return error identifying mismatched file, expected digest, and observed digest
    - _Requirements: 10.2, 10.3, 17.4_

  - [x] 6.3 Update `materialize` to call `verify_integrity` when `integrity.json` is present
    - After unpacking, check for `integrity.json`; if present, verify; if absent, proceed with warning
    - _Requirements: 10.2, 10.3, 10.4_

- [ ] 7. Implement repropack-git snapshot and delta functions
  - [x] 7.1 Add `capture_git_snapshot` function
    - Capture commit SHA, dirty status, changed paths, untracked paths, optional worktree patch
    - Write worktree patch to `git/{label}-worktree.patch` if dirty
    - Return `GitSnapshotOutcome` with snapshot and omissions
    - _Requirements: 1.1, 2.1_

  - [x] 7.2 Add `compute_capture_delta` function
    - Compute set differences between pre and post snapshots
    - `newly_dirty_paths` = post.changed_paths − pre.changed_paths
    - `newly_untracked_paths` = post.untracked_paths − pre.untracked_paths
    - `newly_modified_paths` = intersection of pre and post changed_paths
    - _Requirements: 3.1_

  - [x] 7.3 Write property test for capture delta correctness (Property 1)
    - **Property 1: Capture delta is correct set difference**
    - Generate arbitrary `GitSnapshot` pairs, verify delta is correct set difference
    - **Validates: Requirements 3.1**

- [x] 8. Checkpoint — pack and git layers
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 9. Implement repropack-capture pre/post orchestration, evidence digests, size caps, and integrity envelope
  - [x] 9.1 Add pre-run and post-run git snapshot calls to capture flow
    - Call `capture_git_snapshot(repo_root, git_out_dir, "pre")` before command execution
    - Call `capture_git_snapshot(repo_root, git_out_dir, "post")` after command execution
    - Compute delta via `compute_capture_delta` when both succeed
    - Write `git/capture-delta.json` to packet directory
    - Record omissions with kind `git_pre`, `git_post`, or `capture_delta` on failure
    - Store results in `manifest.git.git_pre`, `manifest.git.git_post`, `manifest.git.capture_delta`
    - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 3.4_

  - [x] 9.2 Compute and store stdout/stderr evidence digests
    - After command execution, compute SHA-256 of stdout and stderr byte buffers using `sha256_bytes`
    - Store as `execution.stdout_sha256` and `execution.stderr_sha256` in manifest
    - _Requirements: 6.1_

  - [x] 9.3 Implement size caps in artifact collection
    - Add `size_caps: SizeCaps` to `CaptureOptions` with default
    - Before copying a file, check against `max_file_bytes`; truncate and record `Omission(kind: "truncated")` if exceeded
    - Track cumulative size; stop and record `Omission(kind: "packet_size_exceeded")` if total exceeds `max_packet_bytes`
    - _Requirements: 11.1, 11.2, 11.3, 11.4_

  - [x] 9.4 Generate integrity envelope after staging
    - After all files are staged (including manifest.json), iterate directory, compute SHA-256 and size for each file except `integrity.json` itself
    - Write `integrity.json` as `Vec<IntegrityEntry>` to packet root
    - _Requirements: 10.1_

  - [x] 9.5 Write property test for per-file size cap (Property 10)
    - **Property 10: Per-file size cap truncates and records omission**
    - **Validates: Requirements 11.1, 11.2**

  - [x] 9.6 Write property test for total packet size cap (Property 11)
    - **Property 11: Total packet size cap stops capture and records omission**
    - **Validates: Requirements 11.3, 11.4**

- [ ] 10. Implement repropack-replay minimal env baseline, env classification, evidence comparison, and delta drift
  - [x] 10.1 Replace host env inheritance with minimal baseline
    - Use `command.env_clear()` then inject only `manifest.environment.allowed_vars` and `options.set_env`
    - `set_env` values take precedence for keys in both
    - Record excluded host vars as `DriftItem(subject: "env_excluded:<KEY>", severity: info)`
    - _Requirements: 4.1, 4.2, 4.3, 4.4_

  - [x] 10.2 Implement `--inherit-env` fallback
    - Add `inherit_env: bool` to `ReplayOptions`
    - When true, inherit full host env (current behavior) and record `DriftItem(subject: "env_inherited", severity: warning)`
    - _Requirements: 4.5_

  - [x] 10.3 Implement environment variable classification
    - Build `EnvClassification` with `restored`, `overridden`, `inherited` arrays
    - Keys in manifest but not in set_env → `restored`
    - Keys in both manifest and set_env → `overridden`
    - Keys from host env when `--inherit-env` → `inherited`
    - Store in `receipt.env_classification`
    - _Requirements: 5.1, 5.2, 5.3_

  - [x] 10.4 Implement stdout/stderr evidence digest comparison
    - After command execution, compute SHA-256 of observed stdout and stderr
    - Compare against `manifest.execution.stdout_sha256` and `stderr_sha256`
    - Add `DriftItem(subject: "stdout_digest"/"stderr_digest", severity: warning)` on mismatch
    - _Requirements: 6.2, 6.3, 6.4_

  - [x] 10.5 Implement output artifact digest comparison
    - For each output in `manifest.outputs`, recompute digest at `restore_path` after command execution
    - Add `DriftItem(subject: "output_digest:<path>", severity: warning)` on mismatch
    - Add `DriftItem(subject: "output_missing:<path>", severity: error)` when file missing
    - Set `receipt.matched_outputs` to true only when all match and none missing
    - _Requirements: 7.1, 7.2, 7.3, 7.4_

  - [x] 10.6 Implement same-exit-different-evidence detection
    - When exit codes match but any evidence digest differs, set `receipt.status = Mismatched` and add note "exit code matched but evidence diverged"
    - `receipt.matched` is true only when exit code AND all evidence digests match
    - _Requirements: 8.1, 8.2_

  - [x] 10.7 Implement capture delta drift comparison
    - When manifest contains `capture_delta`, compute replay's own pre/post delta after running command
    - Compare replay delta against manifest delta; add `DriftItem(subject: "capture_delta", severity: warning)` on difference
    - Skip comparison when manifest has no `capture_delta`
    - _Requirements: 18.1, 18.2, 18.3_

  - [x] 10.8 Write property test for replay env baseline (Property 2)
    - **Property 2: Replay environment baseline contains only declared variables**
    - **Validates: Requirements 4.1, 4.2, 4.3**

  - [x] 10.9 Write property test for env classification partition (Property 3)
    - **Property 3: Environment classification is a disjoint partition**
    - **Validates: Requirements 5.1, 5.2**

  - [x] 10.10 Write property test for evidence digest drift (Property 4)
    - **Property 4: Evidence digest mismatch produces drift**
    - **Validates: Requirements 6.3, 6.4, 7.2**

  - [x] 10.11 Write property test for matched_outputs conjunction (Property 5)
    - **Property 5: matched_outputs is the conjunction of all output comparisons**
    - **Validates: Requirements 7.4**

  - [x] 10.12 Write property test for matched = exit AND evidence (Property 6)
    - **Property 6: Receipt matched equals exit code match AND evidence match**
    - **Validates: Requirements 8.1, 8.2**

  - [x] 10.13 Write property test for capture delta drift comparison (Property 17)
    - **Property 17: Capture delta drift comparison**
    - **Validates: Requirements 18.2**

- [x] 11. Checkpoint — capture and replay orchestration
  - Ensure all tests pass, ask the user if questions arise.

- [x] 12. Update repropack-render for new receipt fields
  - Extend `render_receipt_markdown` to render `env_classification` section (restored/overridden/inherited counts and keys), `matched_outputs` status, and evidence digest drift items
  - _Requirements: 5.3, 7.4, 8.1_

- [ ] 13. Update repropack-cli for new flags
  - [x] 13.1 Add `--inherit-env` flag to replay subcommand
    - Wire to `ReplayOptions.inherit_env`
    - _Requirements: 4.5_

  - [x] 13.2 Add `--max-file-size` and `--max-packet-size` flags to capture subcommand
    - Default values: 52428800 (50 MiB) and 524288000 (500 MiB)
    - Wire to `CaptureOptions.size_caps`
    - _Requirements: 11.1, 11.3_

- [x] 14. Checkpoint — render and CLI
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 15. Property tests for repropack-model serde round-trips and schema conformance
  - [x] 15.1 Write property test for manifest serde round-trip (Property 12)
    - **Property 12: Manifest serde round-trip and schema conformance**
    - Create `arb_packet_manifest()` generator producing valid manifests with all v0.2 fields
    - Verify serialize → deserialize produces equal value
    - Verify serialized JSON validates against manifest schema
    - **Validates: Requirements 12.4, 15.1, 15.3**

  - [x] 15.2 Write property test for receipt serde round-trip (Property 13)
    - **Property 13: Receipt serde round-trip and schema conformance**
    - Create `arb_replay_receipt()` generator producing valid receipts with env_classification and matched_outputs
    - Verify serialize → deserialize produces equal value
    - Verify serialized JSON validates against receipt schema
    - **Validates: Requirements 12.5, 15.2, 15.4**

  - [x] 15.3 Write property test for schema rejection of invalid JSON (Property 7)
    - **Property 7: Schema validation rejects non-conforming JSON**
    - Generate JSON with wrong schema_version, missing required fields, wrong types
    - Verify `validate_manifest` / `validate_receipt` return errors with validation path
    - **Validates: Requirements 9.1, 9.2, 9.4, 17.2, 17.3**

- [ ] 16. Property tests for repropack-pack round-trip, determinism, integrity, and path traversal
  - [x] 16.1 Write property test for pack/unpack round-trip (Property 14)
    - **Property 14: Pack/unpack round-trip preserves all file contents**
    - Create `arb_directory_tree()` generator producing temp dirs with random files
    - Verify pack → unpack produces identical paths and file contents (SHA-256 comparison)
    - **Validates: Requirements 14.1, 14.2**

  - [x] 16.2 Write property test for pack determinism (Property 15)
    - **Property 15: Pack determinism**
    - Pack same directory twice, verify archives are byte-identical
    - **Validates: Requirements 14.3**

  - [x] 16.3 Write property test for path traversal rejection (Property 16)
    - **Property 16: Path traversal rejection**
    - Generate tar entries with `..` components, verify `unpack_rpk` returns error
    - **Validates: Requirements 17.1**

  - [x] 16.4 Write property test for integrity envelope completeness (Property 8)
    - **Property 8: Integrity envelope lists all packet files except itself**
    - **Validates: Requirements 10.1**

  - [x] 16.5 Write property test for integrity mismatch rejection (Property 9)
    - **Property 9: Integrity mismatch is rejected on materialize**
    - **Validates: Requirements 10.3, 17.4**

- [ ] 17. Integration tests — temp-repo scenario suite
  - [x] 17.1 Capture clean commit failure scenario
    - Create temp Git repo, commit a failing script, run capture
    - Assert manifest contains correct commit SHA, exit code, and changed paths
    - _Requirements: 13.1_

  - [x] 17.2 Capture and replay match scenario
    - Capture a packet, replay it, assert `receipt.matched == true` and `drift` is empty
    - _Requirements: 13.2_

  - [x] 17.3 Modified environment replay scenario
    - Capture a packet, modify replay environment, replay, assert drift items present
    - _Requirements: 13.3_

  - [x] 17.4 Modified output replay scenario
    - Capture a packet with outputs, modify an output file, replay, assert `matched_outputs == false`
    - _Requirements: 13.4_

  - [x] 17.5 Capture delta drift replay scenario
    - Capture with pre/post delta, replay, assert delta drift comparison works
    - _Requirements: 18.1_

- [x] 18. CLI smoke tests
  - [x] 18.1 `repropack --help` exits 0 and contains expected subcommands
    - Assert output contains "capture", "inspect", "replay", "unpack"
    - _Requirements: 16.1_

  - [x] 18.2 `repropack capture --help` exits 0
    - _Requirements: 16.2_

  - [x] 18.3 `repropack inspect` with valid packet exits 0 and shows packet ID
    - _Requirements: 16.3_

  - [x] 18.4 `repropack capture` with no command exits non-zero with error
    - _Requirements: 16.4_

- [x] 19. Malformed packet rejection tests
  - [x] 19.1 Path traversal archive entry is rejected
    - Create archive with `../escape` entry, verify `unpack_rpk` returns error
    - _Requirements: 17.1_

  - [x] 19.2 Unrecognized schema_version is rejected
    - Create manifest with wrong `schema_version`, verify `read_from_path` returns error
    - _Requirements: 17.2_

  - [x] 19.3 Missing required fields are rejected
    - Create manifest missing required fields, verify schema validator returns error with field path
    - _Requirements: 17.3_

  - [x] 19.4 Corrupted integrity digest is rejected
    - Create packet with tampered file, verify `materialize` returns error identifying the file
    - _Requirements: 17.4_

- [x] 20. Final checkpoint — full CI
  - Ensure all tests pass via `cargo xtask ci-full`, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Dependency order: model (tasks 2–4) → pack/git (tasks 6–7) → capture/replay (tasks 9–10) → render/cli (tasks 12–13) → tests (tasks 15–19)
- Each task references specific requirements for traceability
- Property tests reference their design property number
- Checkpoints at tasks 5, 8, 11, 14, and 20 ensure incremental validation
