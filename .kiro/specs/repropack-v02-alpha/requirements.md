# Requirements Document

## Introduction

ReproPack v0.2 alpha hardens the existing scaffold into a compile-checked, testable CLI. The five priority gaps are addressed: pre/post-run capture model, replay environment discipline, real end-to-end tests, artifact comparison beyond exit code, and runtime schema / packet hardening. The goal is a trustworthy alpha where every operator-visible claim is backed by structured evidence and tested invariants.

## Glossary

- **Packet**: A portable `.rpk` archive (or directory) containing a manifest, Git state, execution evidence, environment snapshot, inputs, and outputs produced by a single capture run.
- **Manifest**: The `manifest.json` file inside a Packet. It is the authoritative record of what was captured.
- **Receipt**: The `receipt.json` file produced by replay. It records observed outcomes, drift, and match status.
- **Capture_Engine**: The orchestration logic in `repropack-capture` that runs a command, records Git and environment state, collects artifacts, and assembles a Packet.
- **Replay_Engine**: The orchestration logic in `repropack-replay` that materializes a Packet, restores repo state, reruns the command, and emits a Receipt.
- **CLI**: The `repropack-cli` binary that exposes capture, inspect, replay, unpack, and emit subcommands to the operator.
- **Drift_Item**: A structured record in the Receipt describing a specific divergence between recorded and observed state during replay.
- **Omission**: A structured record in the Manifest describing something the Capture_Engine could not or chose not to capture.
- **Pre_Run_State**: The Git commit SHA, dirty status, changed paths, and worktree patch captured before the predicate command executes.
- **Post_Run_State**: The Git dirty status, changed paths, and worktree patch captured after the predicate command executes.
- **Capture_Delta**: A structured diff summarizing what changed between Pre_Run_State and Post_Run_State (new dirty files, modified paths, exit artifacts).
- **Evidence_Digest**: A SHA-256 digest of stdout, stderr, or an output artifact used for comparison beyond exit code.
- **Integrity_Envelope**: A detached JSON object listing the SHA-256 digest and size of every file in the Packet, used for read-time validation.
- **Schema_Validator**: The component that validates `manifest.json` and `receipt.json` against their JSON Schema definitions at read time.
- **Env_Baseline**: The minimal, deterministic set of environment variables injected into the replay command, replacing full host-env inheritance.
- **Size_Cap**: A configurable maximum byte size for individual captured artifacts and for the total Packet.
- **Truncation_Record**: An Omission entry recording that an artifact was truncated to fit within a Size_Cap, including original size and retained size.
- **Scenario_Suite**: A collection of integration tests that create temporary Git repositories, run capture and replay, and assert on Manifest and Receipt contents.
- **Property_Test**: A test that uses randomized inputs to verify an invariant holds for all generated cases (e.g., serde round-trip, pack/unpack determinism).

## Requirements

### Requirement 1: Pre-Run Git State Capture

**User Story:** As a developer, I want the Capture_Engine to record Git state before running my command, so that the Packet reflects the repo at the moment the predicate was invoked.

#### Acceptance Criteria

1. WHEN a capture is initiated, THE Capture_Engine SHALL record Pre_Run_State (commit SHA, ref name, dirty status, changed paths, worktree patch) before executing the predicate command.
2. THE Manifest SHALL store Pre_Run_State in a `git_pre` field within the `git` object.
3. WHEN Pre_Run_State capture fails, THE Capture_Engine SHALL record an Omission with kind `git_pre` and continue with the capture.

### Requirement 2: Post-Run Git State Capture

**User Story:** As a developer, I want the Capture_Engine to record Git state after running my command, so that I can see what the predicate changed on disk.

#### Acceptance Criteria

1. WHEN the predicate command finishes, THE Capture_Engine SHALL record Post_Run_State (dirty status, changed paths, worktree patch) from the same repository root.
2. THE Manifest SHALL store Post_Run_State in a `git_post` field within the `git` object.
3. WHEN Post_Run_State capture fails, THE Capture_Engine SHALL record an Omission with kind `git_post` and continue with the capture.

### Requirement 3: Capture Delta Computation

**User Story:** As a developer, I want the Packet to contain an explicit delta between pre-run and post-run state, so that I can quickly see what the predicate changed.

#### Acceptance Criteria

1. WHEN both Pre_Run_State and Post_Run_State are available, THE Capture_Engine SHALL compute a Capture_Delta containing newly dirty paths, newly modified paths, and newly untracked paths.
2. THE Manifest SHALL store the Capture_Delta in a `capture_delta` field within the `git` object.
3. THE Capture_Engine SHALL write the Capture_Delta to `git/capture-delta.json` inside the Packet directory.
4. IF either Pre_Run_State or Post_Run_State is missing, THEN THE Capture_Engine SHALL omit the Capture_Delta and record an Omission with kind `capture_delta`.

### Requirement 4: Replay Minimal Environment Baseline

**User Story:** As a developer, I want replay to start from a minimal environment rather than inheriting the full host env, so that replay results are more reproducible.

#### Acceptance Criteria

1. THE Replay_Engine SHALL construct the replay command environment starting from an empty set of variables, not from the host process environment.
2. THE Replay_Engine SHALL inject only the variables recorded in the Manifest `environment.allowed_vars` map into the replay command environment.
3. WHEN the operator provides `--set-env` overrides, THE Replay_Engine SHALL apply those overrides after injecting recorded variables.
4. THE Replay_Engine SHALL record each host environment variable that was present but not injected as a Drift_Item with subject `env_excluded:<KEY>` and severity `info`.
5. WHERE the operator passes a `--inherit-env` flag, THE Replay_Engine SHALL fall back to the current host environment inheritance behavior and record a Drift_Item with subject `env_inherited` and severity `warning`.

### Requirement 5: Replay Environment Variable Classification

**User Story:** As a developer, I want the Receipt to distinguish between restored, overridden, and inherited environment variables, so that I can audit what shaped the replay.

#### Acceptance Criteria

1. THE Receipt SHALL include an `env_classification` object with three arrays: `restored` (from Manifest), `overridden` (from `--set-env`), and `inherited` (from host when `--inherit-env` is used).
2. WHEN a variable appears in both the Manifest and `--set-env`, THE Replay_Engine SHALL use the `--set-env` value and classify the variable as `overridden`.
3. THE Receipt `env_classification` field SHALL be validated against the receipt JSON Schema.

### Requirement 6: Stdout and Stderr Digest Comparison

**User Story:** As a developer, I want replay to compare stdout and stderr digests against the capture, so that I can detect output divergence even when exit codes match.

#### Acceptance Criteria

1. THE Capture_Engine SHALL compute and store SHA-256 Evidence_Digests for `exec/stdout.log` and `exec/stderr.log` in the Manifest `execution` object as `stdout_sha256` and `stderr_sha256`.
2. WHEN replay completes command execution, THE Replay_Engine SHALL compute SHA-256 digests of the observed stdout and stderr.
3. WHEN the observed stdout digest differs from the recorded `stdout_sha256`, THE Replay_Engine SHALL add a Drift_Item with subject `stdout_digest`, severity `warning`, the expected digest, and the observed digest.
4. WHEN the observed stderr digest differs from the recorded `stderr_sha256`, THE Replay_Engine SHALL add a Drift_Item with subject `stderr_digest`, severity `warning`, the expected digest, and the observed digest.

### Requirement 7: Output Artifact Digest Comparison

**User Story:** As a developer, I want replay to compare output artifact digests against the capture, so that I can detect changed build artifacts even when the exit code is the same.

#### Acceptance Criteria

1. WHEN the Manifest `outputs` array contains indexed files with SHA-256 digests, THE Replay_Engine SHALL recompute digests for each output file found at its `restore_path` after command execution.
2. WHEN an output file digest differs from the recorded digest, THE Replay_Engine SHALL add a Drift_Item with subject `output_digest:<PACKET_PATH>`, severity `warning`, the expected digest, and the observed digest.
3. WHEN an output file recorded in the Manifest is missing after replay, THE Replay_Engine SHALL add a Drift_Item with subject `output_missing:<PACKET_PATH>` and severity `error`.
4. THE Receipt SHALL include a `matched_outputs` boolean that is true only when all recorded output digests match and no recorded outputs are missing.

### Requirement 8: Same-Exit-Different-Evidence Reporting

**User Story:** As a developer, I want the Receipt to flag when exit codes match but evidence diverges, so that I do not mistake a changed failure for a stable reproduction.

#### Acceptance Criteria

1. WHEN the observed exit code matches the recorded exit code and at least one Evidence_Digest (stdout, stderr, or output artifact) differs, THE Replay_Engine SHALL set the Receipt `status` to `mismatched` and add a note stating "exit code matched but evidence diverged".
2. THE Receipt `matched` field SHALL be true only when the exit code matches and all Evidence_Digests match.

### Requirement 9: Schema Validation at Read Time

**User Story:** As a developer, I want manifest and receipt JSON to be validated against their schemas when read, so that malformed packets are rejected early.

#### Acceptance Criteria

1. WHEN `PacketManifest::read_from_path` is called, THE Schema_Validator SHALL validate the JSON against `manifest-v1.schema.json` before deserialization.
2. WHEN `ReplayReceipt::read_from_path` is called, THE Schema_Validator SHALL validate the JSON against `receipt-v1.schema.json` before deserialization.
3. IF validation fails, THEN THE Schema_Validator SHALL return an error containing the validation failure path and message.
4. THE Schema_Validator SHALL accept documents whose `schema_version` matches the expected version string and reject documents with an unrecognized `schema_version`.

### Requirement 10: Integrity Envelope

**User Story:** As a developer, I want the Packet to contain an integrity envelope, so that I can verify no files were tampered with or corrupted after capture.

#### Acceptance Criteria

1. THE Capture_Engine SHALL generate an `integrity.json` file listing the relative path, SHA-256 digest, and size in bytes of every file in the Packet except `integrity.json` itself.
2. WHEN materializing a Packet, THE `materialize` function in `repropack-pack` SHALL verify each file against `integrity.json` if the envelope is present.
3. IF any file fails integrity verification, THEN THE `materialize` function SHALL return an error identifying the mismatched file, expected digest, and observed digest.
4. WHEN `integrity.json` is absent, THE `materialize` function SHALL proceed without verification and log a warning note.

### Requirement 11: Artifact Size Caps and Truncation

**User Story:** As a developer, I want the Capture_Engine to enforce size limits on captured artifacts, so that packets remain portable and do not consume unbounded disk space.

#### Acceptance Criteria

1. THE Capture_Engine SHALL enforce a configurable per-file Size_Cap (default 50 MiB) on each artifact written to the Packet.
2. WHEN an artifact exceeds the per-file Size_Cap, THE Capture_Engine SHALL truncate the artifact to the Size_Cap and record a Truncation_Record Omission with kind `truncated`, the original size, and the retained size.
3. THE Capture_Engine SHALL enforce a configurable total Packet Size_Cap (default 500 MiB) across all artifacts.
4. WHEN the total Packet size would exceed the total Size_Cap, THE Capture_Engine SHALL stop capturing additional artifacts and record an Omission with kind `packet_size_exceeded`.

### Requirement 12: Manifest and Receipt Schema Updates

**User Story:** As a developer, I want the JSON schemas to reflect the new fields added in v0.2, so that tooling can validate packets produced by the updated Capture_Engine.

#### Acceptance Criteria

1. THE `manifest-v1.schema.json` SHALL include definitions for `git_pre`, `git_post`, `capture_delta`, `stdout_sha256`, `stderr_sha256`, and `integrity.json` reference.
2. THE `receipt-v1.schema.json` SHALL include definitions for `env_classification`, `matched_outputs`, and updated `matched` semantics.
3. THE schema `schema_version` strings SHALL remain `repropack.manifest.v1` and `repropack.receipt.v1` because the changes are additive and backward-compatible.
4. FOR ALL valid Manifest JSON documents, serializing then deserializing through `PacketManifest` SHALL produce a semantically equivalent object (round-trip property).
5. FOR ALL valid Receipt JSON documents, serializing then deserializing through `ReplayReceipt` SHALL produce a semantically equivalent object (round-trip property).

### Requirement 13: Temp-Repo Integration Test Suite

**User Story:** As a developer, I want integration tests that create temporary Git repositories and run capture/replay end-to-end, so that I can trust the full pipeline.

#### Acceptance Criteria

1. THE Scenario_Suite SHALL include a test that creates a temporary Git repo, commits a failing script, runs capture, and asserts the Manifest contains the correct commit SHA, exit code, and changed paths.
2. THE Scenario_Suite SHALL include a test that captures a Packet, replays it, and asserts the Receipt `matched` field is true and `drift` is empty.
3. THE Scenario_Suite SHALL include a test that captures a Packet, modifies the replay environment, replays, and asserts the Receipt contains appropriate Drift_Items.
4. THE Scenario_Suite SHALL include a test that captures a Packet with outputs, replays after modifying an output file, and asserts `matched_outputs` is false.

### Requirement 14: Pack/Unpack Round-Trip Property Tests

**User Story:** As a developer, I want property tests proving that pack then unpack preserves all file contents and metadata, so that I can trust the archive layer.

#### Acceptance Criteria

1. FOR ALL directory trees containing files with arbitrary byte contents, packing with `pack_dir` then unpacking with `unpack_rpk` SHALL produce a directory tree with identical relative paths and identical file contents.
2. FOR ALL directory trees, the SHA-256 digest of each file after unpack SHALL match the digest of the corresponding file before pack.
3. FOR ALL directory trees, packing the same source directory twice SHALL produce archives with identical content (determinism property).

### Requirement 15: Manifest and Receipt Serde Round-Trip Property Tests

**User Story:** As a developer, I want property tests proving that serializing and deserializing manifests and receipts preserves all fields, so that I can trust the model layer.

#### Acceptance Criteria

1. FOR ALL valid `PacketManifest` values, serializing to JSON then deserializing SHALL produce a value equal to the original.
2. FOR ALL valid `ReplayReceipt` values, serializing to JSON then deserializing SHALL produce a value equal to the original.
3. FOR ALL valid `PacketManifest` values, the serialized JSON SHALL validate against `manifest-v1.schema.json`.
4. FOR ALL valid `ReplayReceipt` values, the serialized JSON SHALL validate against `receipt-v1.schema.json`.

### Requirement 16: CLI Smoke Tests

**User Story:** As a developer, I want CLI smoke tests that verify each subcommand exits correctly and produces expected output, so that I can catch regressions in the operator surface.

#### Acceptance Criteria

1. THE CLI smoke tests SHALL verify that `repropack --help` exits with code 0 and outputs text containing "capture", "inspect", "replay", and "unpack".
2. THE CLI smoke tests SHALL verify that `repropack capture --help` exits with code 0.
3. THE CLI smoke tests SHALL verify that `repropack inspect` with a valid Packet exits with code 0 and outputs a summary containing the packet ID.
4. THE CLI smoke tests SHALL verify that `repropack capture` with no command argument exits with a non-zero code and outputs an error message.

### Requirement 17: Malformed Packet Rejection Tests

**User Story:** As a developer, I want tests that verify malformed packets are rejected with clear errors, so that I can trust the system handles bad input gracefully.

#### Acceptance Criteria

1. WHEN a Packet archive contains a path traversal entry (e.g., `../escape`), THE `unpack_rpk` function SHALL return an error and not write any file outside the target directory.
2. WHEN a Packet `manifest.json` has an unrecognized `schema_version`, THE `PacketManifest::read_from_path` function SHALL return an error.
3. WHEN a Packet `manifest.json` is missing required fields, THE Schema_Validator SHALL return an error identifying the missing field.
4. WHEN a Packet `integrity.json` lists a digest that does not match the actual file, THE `materialize` function SHALL return an error identifying the corrupted file.

### Requirement 18: Replay Drift for Pre/Post State

**User Story:** As a developer, I want replay to compare its own pre/post Git state against the Manifest's Capture_Delta, so that I can see whether the predicate had the same effect during replay.

#### Acceptance Criteria

1. WHEN the Manifest contains a Capture_Delta, THE Replay_Engine SHALL compute its own pre/post Git delta after running the command.
2. WHEN the replay delta differs from the recorded Capture_Delta, THE Replay_Engine SHALL add a Drift_Item with subject `capture_delta`, severity `warning`, and a description of the difference.
3. WHEN the Manifest does not contain a Capture_Delta, THE Replay_Engine SHALL skip delta comparison and not emit a Drift_Item for it.
