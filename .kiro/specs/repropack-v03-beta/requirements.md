# Requirements Document

## Introduction

ReproPack v0.3 beta graduates the v0.2 alpha into a tool that delivers on its core promise: a captured failure packet that replays cleanly, explains itself to operators, and can be safely shared externally. The release is organized into five priority tiers: (1) fix replay cleanliness issues that prevent `matched == true` on happy paths, (2) add operator diagnostic surfaces (`doctor`, `explain`, `shell`), (3) add the safe-sharing wedge (`scrub --public`), (4) harden the CI bridge (`fetch gh`, `gh summarize`), and (5) introduce configuration profiles (`.repropack.toml`). No containers, service backends, plugin systems, or dashboard layers are in scope.

## Glossary

- **Packet**: A portable `.rpk` archive (or directory) containing a manifest, Git state, execution evidence, environment snapshot, inputs, and outputs produced by a single capture run.
- **Manifest**: The `manifest.json` file inside a Packet. It is the authoritative record of what was captured.
- **Receipt**: The `receipt.json` file produced by replay. It records observed outcomes, drift, and match status.
- **Capture_Engine**: The orchestration logic in `repropack-capture` that runs a command, records Git and environment state, collects artifacts, and assembles a Packet.
- **Replay_Engine**: The orchestration logic in `repropack-replay` that materializes a Packet, restores repo state, reruns the command, and emits a Receipt.
- **CLI**: The `repropack-cli` binary that exposes subcommands to the operator.
- **Drift_Item**: A structured record in the Receipt describing a specific divergence between recorded and observed state during replay.
- **Omission**: A structured record in the Manifest describing something the Capture_Engine could not or chose not to capture.
- **Replay_Support_Dir**: The `.repropack-replay/` directory inside the replay workdir where replay logs, receipt, and temporary files are written.
- **Capture_Delta**: A structured diff summarizing what changed between pre-run and post-run Git state.
- **Evidence_Digest**: A SHA-256 digest of stdout, stderr, or an output artifact used for comparison beyond exit code.
- **Scrub_Engine**: The component that produces a redacted copy of a Packet by removing or replacing sensitive content according to a policy, emitting a redaction report alongside the scrubbed Packet.
- **Redaction_Report**: A JSON document listing every field or file that was removed or replaced during scrubbing, with the reason for each redaction.
- **Doctor_Report**: A structured diagnostic summary produced by `repropack doctor` that assesses a Packet's completeness, redactions, drift sources, toolchain mismatches, and replay-worthiness.
- **Config_File**: A `.repropack.toml` file at the repository root that provides default capture and replay settings.
- **Profile**: A named configuration section within the Config_File (e.g., `ci`, `local`, `triage`) that overrides default settings.
- **Bundle**: A Git bundle file (`repo.bundle`) included in the Packet for replay-time repository materialization.
- **Root_Commit**: The first commit in a Git repository, which has no parent and requires special handling for `git bundle create`.

## Requirements

### Requirement 1: Replay Log Isolation from Delta Computation

**User Story:** As a developer, I want replay logs to be excluded from the repo-side-effect comparison, so that the replay engine does not report its own log files as unexpected worktree changes.

#### Acceptance Criteria

1. THE Replay_Engine SHALL write `stdout.log` and `stderr.log` to a location outside the Git-tracked worktree, or SHALL exclude the Replay_Support_Dir from delta computation.
2. WHEN the Replay_Engine computes a post-run Git snapshot for delta comparison, THE Replay_Engine SHALL exclude all paths under the Replay_Support_Dir from the `changed_paths` and `untracked_paths` arrays.
3. WHEN a replay completes on a clean predicate (no side effects), THE Receipt SHALL report `matched == true` with an empty `capture_delta` drift, provided exit code and all evidence digests also match.

### Requirement 2: Bundle Creation for Root Commits

**User Story:** As a developer, I want `git bundle create` to succeed for repositories with only a root commit, so that capture does not fail or require manual workarounds on simple repos.

#### Acceptance Criteria

1. WHEN `git bundle create <bundle> <sha>` fails with an empty-bundle error, THE Capture_Engine SHALL retry using `git bundle create <bundle> --all` as a fallback.
2. WHEN the fallback succeeds, THE Capture_Engine SHALL record the bundle path in the Manifest without recording an Omission for the bundle.
3. WHEN both the primary and fallback attempts fail, THE Capture_Engine SHALL record an Omission with kind `bundle` and the failure reason.
4. FOR ALL Git repositories with at least one commit, THE Capture_Engine SHALL produce a Packet containing a valid `git/repo.bundle` file or an explicit Omission explaining why the bundle is absent.


### Requirement 3: Env-Excluded Drift Noise Collapse

**User Story:** As a developer, I want the replay output to collapse per-variable env-excluded drift into a summary count, so that the receipt and human output are not dominated by dozens of irrelevant host environment variables.

#### Acceptance Criteria

1. THE Replay_Engine SHALL replace individual `env_excluded:<KEY>` Drift_Items with a single summary Drift_Item whose subject is `env_excluded_summary`, severity is `info`, and whose `observed` field contains the count of excluded variables.
2. THE Receipt SHALL store the full list of excluded variable names in a new optional `env_excluded_keys` array field for programmatic access.
3. WHEN the operator passes a `--verbose` flag to the replay subcommand, THE CLI SHALL render the full list of excluded variable names in the human-readable output.
4. WHEN the operator does not pass `--verbose`, THE CLI SHALL render only the summary count of excluded variables.

### Requirement 4: End-to-End Replay Match on Happy Path

**User Story:** As a developer, I want at least one standard capture-then-replay scenario to produce `matched == true` with zero drift items of severity `warning` or `error`, so that the core promise of ReproPack is demonstrably true.

#### Acceptance Criteria

1. THE Scenario_Suite SHALL include a test that creates a temporary Git repo, commits a deterministic passing script, captures a Packet, replays the Packet, and asserts `receipt.matched == true`.
2. THE test in acceptance criterion 1 SHALL assert that the Receipt contains zero Drift_Items with severity `warning` or `error`.
3. THE test in acceptance criterion 1 SHALL assert that `receipt.status` is `matched`.
4. THE test in acceptance criterion 1 SHALL assert that `receipt.matched_outputs` is either `true` or absent (when no outputs are captured).

### Requirement 5: Tool Fingerprint Path Normalization

**User Story:** As a developer, I want tool version comparison to ignore path and prompt differences in version strings, so that replay does not report false-positive tool drift caused by different installation paths.

#### Acceptance Criteria

1. WHEN comparing tool version strings, THE Replay_Engine SHALL normalize version output by extracting only the semantic version component (e.g., `1.78.0`) from the full version string before comparison.
2. WHEN the normalized version components match but the full version strings differ, THE Replay_Engine SHALL not emit a Drift_Item for that tool.
3. WHEN the normalized version components differ, THE Replay_Engine SHALL emit a Drift_Item with subject `tool_version:<TOOL>`, severity `warning`, and the full version strings as `expected` and `observed`.

### Requirement 6: Doctor Subcommand

**User Story:** As a developer, I want a `repropack doctor` command that summarizes a Packet's completeness and replay-worthiness, so that I can quickly assess whether a packet is useful before attempting replay.

#### Acceptance Criteria

1. WHEN `repropack doctor <packet>` is invoked, THE CLI SHALL materialize the Packet and produce a Doctor_Report.
2. THE Doctor_Report SHALL list all Omissions from the Manifest, grouped by kind, with counts.
3. THE Doctor_Report SHALL list all redacted environment variable keys.
4. THE Doctor_Report SHALL list all tool version entries and flag any that are missing from the current host.
5. THE Doctor_Report SHALL assess replay-worthiness as one of: `ready` (bundle present, no critical omissions, policy allows replay), `degraded` (bundle present but omissions or redactions reduce fidelity), or `blocked` (no bundle, or policy is disabled).
6. THE Doctor_Report SHALL be rendered as human-readable text by default, and as JSON when the `--json` flag is passed.
7. THE CLI SHALL exit with code 0 when the Packet is `ready` or `degraded`, and exit with code 1 when the Packet is `blocked`.

### Requirement 7: Explain Subcommand

**User Story:** As a developer, I want a `repropack explain <receipt>` command that translates digest mismatches and drift items into human-readable reasons, so that I can understand why a replay did not match without manually comparing SHA-256 hashes.

#### Acceptance Criteria

1. WHEN `repropack explain <receipt>` is invoked, THE CLI SHALL read the Receipt and produce a human-readable explanation of each Drift_Item.
2. WHEN a Drift_Item has subject `stdout_digest` or `stderr_digest`, THE Explain_Output SHALL state that the command produced different output and show the first differing bytes if the original log files are accessible.
3. WHEN a Drift_Item has subject `output_digest:<PATH>`, THE Explain_Output SHALL state which output file changed and its expected versus observed digest.
4. WHEN a Drift_Item has subject `output_missing:<PATH>`, THE Explain_Output SHALL state which output file was expected but not found after replay.
5. WHEN a Drift_Item has subject `capture_delta`, THE Explain_Output SHALL list the specific path differences between the recorded and observed deltas.
6. WHEN a Drift_Item has subject `tool_version:<TOOL>`, THE Explain_Output SHALL state the tool name, expected version, and observed version.
7. THE CLI SHALL exit with code 0 when the Receipt status is `matched`, and exit with code 1 otherwise.

### Requirement 8: Shell Subcommand

**User Story:** As a developer, I want a `repropack shell <packet>` command that materializes the repo state from a packet and drops me into an interactive shell, so that I can explore the failure environment without rerunning the predicate.

#### Acceptance Criteria

1. WHEN `repropack shell <packet>` is invoked, THE CLI SHALL materialize the Packet into a work directory, restore the Git state (clone bundle, checkout commit, apply patches), and restore captured inputs.
2. THE CLI SHALL set the shell environment to the minimal baseline from the Manifest `environment.allowed_vars`, consistent with the replay environment baseline behavior.
3. THE CLI SHALL launch the user's default shell (from `SHELL` environment variable, falling back to `/bin/sh` on Unix) with the current directory set to the materialized work directory.
4. THE CLI SHALL print a banner message before launching the shell, indicating the packet name, commit SHA, and that the user is in a repropack shell session.
5. WHEN the shell session exits, THE CLI SHALL exit with the shell's exit code.
6. WHERE the operator passes an `--into <DIR>` flag, THE CLI SHALL materialize into the specified directory instead of generating a default name.
7. THE CLI SHALL not execute the predicate command; materialization and shell launch are the only actions.


### Requirement 9: Scrub Subcommand for Safe External Sharing

**User Story:** As a developer, I want a `repropack scrub --public <packet>` command that produces a redacted packet safe for sharing outside my organization, so that I can attach failure evidence to bug reports without leaking secrets.

#### Acceptance Criteria

1. WHEN `repropack scrub --public <packet>` is invoked, THE Scrub_Engine SHALL produce a new Packet with all `environment.allowed_vars` values replaced by the string `[REDACTED]`.
2. THE Scrub_Engine SHALL remove the contents of `exec/stdout.log` and `exec/stderr.log`, replacing each with a placeholder stating the file was redacted, and SHALL update the corresponding `stdout_sha256` and `stderr_sha256` fields in the Manifest to reflect the redacted content.
3. THE Scrub_Engine SHALL remove all files under `inputs/files/` and `outputs/files/`, update the Manifest `inputs` and `outputs` arrays to reflect the removals, and record an Omission with kind `scrubbed` for each removed file.
4. THE Scrub_Engine SHALL preserve the Manifest structure, Git state metadata (commit SHA, ref name, changed paths), execution metadata (exit code, duration, timestamps), and all Omission records.
5. THE Scrub_Engine SHALL generate a Redaction_Report listing every field or file that was modified or removed, with the reason `public_scrub` for each entry.
6. THE Scrub_Engine SHALL write the Redaction_Report to `redaction-report.json` inside the scrubbed Packet.
7. THE Scrub_Engine SHALL regenerate `integrity.json` for the scrubbed Packet so that the integrity envelope remains valid.
8. THE Scrub_Engine SHALL set `replay_fidelity` to `inspect_only` in the scrubbed Manifest, because the scrubbed Packet cannot be replayed.
9. WHEN the `--output` flag is provided, THE Scrub_Engine SHALL write the scrubbed Packet to the specified path; otherwise THE Scrub_Engine SHALL write to `<original_name>-scrubbed.rpk`.
10. THE scrubbed Packet SHALL pass schema validation and integrity verification when read by `repropack inspect`.

### Requirement 10: Redaction Report Schema

**User Story:** As a developer, I want the redaction report to follow a defined schema, so that tooling can programmatically determine what was removed from a scrubbed packet.

#### Acceptance Criteria

1. THE Redaction_Report SHALL be a JSON array of objects, each containing `field_or_path` (string identifying what was redacted), `action` (one of `replaced`, `removed`, `cleared`), and `reason` (string explaining why).
2. THE Redaction_Report SHALL include one entry for each environment variable value that was replaced.
3. THE Redaction_Report SHALL include one entry for each file that was removed from `inputs/files/` or `outputs/files/`.
4. THE Redaction_Report SHALL include entries for `exec/stdout.log` and `exec/stderr.log` when their contents are replaced.
5. FOR ALL scrubbed Packets, the Redaction_Report SHALL be valid JSON and SHALL be listed in the integrity envelope.

### Requirement 11: CI Bridge — Fetch GitHub Artifact

**User Story:** As a developer, I want a `repropack fetch gh <run-id>` command that downloads a packet artifact from a GitHub Actions run, so that I can triage CI failures locally without navigating the GitHub UI.

#### Acceptance Criteria

1. WHEN `repropack fetch gh <run-id>` is invoked, THE CLI SHALL use the GitHub API to list artifacts for the specified workflow run and download the first artifact whose name matches the pattern `repropack-*`.
2. WHEN the `--job <job-name>` flag is provided, THE CLI SHALL filter artifacts to those associated with the specified job.
3. WHEN the `--sha <commit-sha>` flag is provided instead of a run ID, THE CLI SHALL find the most recent workflow run for that commit and download its packet artifact.
4. THE CLI SHALL authenticate using the `GH_TOKEN` or `GITHUB_TOKEN` environment variable.
5. IF no matching artifact is found, THEN THE CLI SHALL exit with code 1 and print a message listing available artifact names.
6. THE CLI SHALL extract the downloaded artifact zip and write the contained `.rpk` file to the current directory, or to the path specified by `--output`.
7. WHEN the `--repo <owner/repo>` flag is provided, THE CLI SHALL target that repository; otherwise THE CLI SHALL infer the repository from the current Git remote.

### Requirement 12: CI Bridge — GitHub Actions Summary

**User Story:** As a developer, I want a `repropack gh summarize <packet>` command that writes a Markdown triage summary suitable for a GitHub Actions job summary, so that CI failures include structured repropack context.

#### Acceptance Criteria

1. WHEN `repropack gh summarize <packet>` is invoked, THE CLI SHALL materialize the Packet and render a Markdown summary containing: packet name, commit SHA, command display, exit code, replay fidelity, omission count, and drift summary (if a receipt is co-located).
2. THE Markdown summary SHALL be formatted for GitHub Actions job summary rendering (using `$GITHUB_STEP_SUMMARY`).
3. WHEN the `--receipt <receipt>` flag is provided, THE CLI SHALL include replay results (matched status, drift items, evidence comparison) in the summary.
4. THE CLI SHALL write the summary to stdout by default, or to the file specified by `--output`.
5. WHEN the `--append-step-summary` flag is provided, THE CLI SHALL append the summary to the file at `$GITHUB_STEP_SUMMARY`.

### Requirement 13: Packet Naming Convention

**User Story:** As a developer, I want a consistent packet naming convention for CI-produced packets, so that artifact retention and lookup are predictable.

#### Acceptance Criteria

1. WHEN the `--name` flag is not provided during capture, THE Capture_Engine SHALL generate a default packet name using the pattern `repropack-<short-sha>-<timestamp>` where `<short-sha>` is the first 8 characters of the commit SHA and `<timestamp>` is a compact UTC timestamp (`YYYYMMDD-HHMMSS`).
2. WHEN the `--name` flag is provided, THE Capture_Engine SHALL use the provided name as the packet name, slugified to contain only lowercase alphanumeric characters and hyphens.
3. THE Capture_Engine SHALL use the packet name as the base filename for the output `.rpk` file (e.g., `<packet-name>.rpk`), replacing the current `<slug>-<uuid>.rpk` pattern.
4. WHEN a file with the computed output name already exists, THE Capture_Engine SHALL append a numeric suffix (e.g., `-1`, `-2`) rather than failing.


### Requirement 14: Configuration File Support

**User Story:** As a developer, I want a `.repropack.toml` configuration file at my repository root, so that I can set default capture and replay options without passing flags on every invocation.

#### Acceptance Criteria

1. WHEN the CLI starts, THE CLI SHALL search for `.repropack.toml` in the current directory and each parent directory up to the Git repository root.
2. WHEN a `.repropack.toml` file is found, THE CLI SHALL parse it and apply its settings as defaults for the current invocation.
3. THE Config_File SHALL support the following top-level keys: `env_allow` (array of glob patterns), `env_deny` (array of glob patterns), `max_file_size` (integer bytes), `max_packet_size` (integer bytes), `format` (string: `rpk` or `dir`), `git_bundle` (string: `auto`, `always`, or `never`), and `replay_policy` (string: `safe`, `confirm`, or `disabled`).
4. WHEN a CLI flag conflicts with a Config_File setting, THE CLI flag SHALL take precedence.
5. IF the `.repropack.toml` file contains an unrecognized key, THEN THE CLI SHALL print a warning naming the unrecognized key and continue execution.
6. IF the `.repropack.toml` file is malformed TOML, THEN THE CLI SHALL exit with code 1 and print an error identifying the parse failure location.

### Requirement 15: Named Configuration Profiles

**User Story:** As a developer, I want named profiles in `.repropack.toml` (e.g., `ci`, `local`, `triage`), so that I can switch between capture presets without editing the config file.

#### Acceptance Criteria

1. THE Config_File SHALL support `[profile.<name>]` sections that override top-level defaults.
2. WHEN the `--profile <name>` flag is passed to any CLI subcommand, THE CLI SHALL merge the named profile's settings over the top-level defaults before applying CLI flags.
3. IF the `--profile` flag references a profile name that does not exist in the Config_File, THEN THE CLI SHALL exit with code 1 and print an error naming the missing profile.
4. THE Config_File SHALL support a `default_profile` top-level key that specifies which profile to use when `--profile` is not passed.
5. WHEN `default_profile` is set and `--profile` is not passed, THE CLI SHALL use the default profile's settings.

### Requirement 16: Profile-Specific Capture Presets

**User Story:** As a developer, I want profiles to support capture presets for common CI runners, so that I can configure environment allow/deny lists appropriate for GitHub Actions, GitLab CI, or local development.

#### Acceptance Criteria

1. THE Config_File profile sections SHALL support all keys available at the top level (`env_allow`, `env_deny`, `max_file_size`, `max_packet_size`, `format`, `git_bundle`, `replay_policy`).
2. WHEN a profile key is present, THE CLI SHALL use the profile value instead of the top-level default for that key.
3. WHEN a profile key is absent, THE CLI SHALL fall back to the top-level default for that key.

### Requirement 17: Config File Pretty-Printer

**User Story:** As a developer, I want a `repropack config show` command that prints the resolved configuration, so that I can verify which settings are active for a given invocation.

#### Acceptance Criteria

1. WHEN `repropack config show` is invoked, THE CLI SHALL locate the Config_File, merge the active profile (if any), and print the resolved configuration as TOML to stdout.
2. WHEN no Config_File is found, THE CLI SHALL print the built-in defaults as TOML.
3. WHEN the `--profile <name>` flag is passed, THE CLI SHALL show the resolved configuration with that profile applied.
4. THE Pretty_Printer SHALL format the TOML output with comments indicating the source of each value (default, config file, or profile).
5. FOR ALL valid Config_File documents, parsing then printing then parsing SHALL produce an equivalent configuration object (round-trip property).

### Requirement 18: Verbose Flag for CLI

**User Story:** As a developer, I want a global `--verbose` flag on the CLI, so that I can get detailed output from any subcommand when troubleshooting.

#### Acceptance Criteria

1. THE CLI SHALL accept a `--verbose` (or `-v`) flag before any subcommand.
2. WHEN `--verbose` is active, THE CLI SHALL render detailed output including full lists of excluded environment variables, individual drift items, and file-level integrity check results.
3. WHEN `--verbose` is not active, THE CLI SHALL render summary-level output with counts instead of full lists.

### Requirement 19: Replay Support Dir Exclusion from Git Status

**User Story:** As a developer, I want the replay engine to ensure that its own support directory does not pollute Git status queries, so that delta comparison is accurate.

#### Acceptance Criteria

1. WHEN the Replay_Engine runs `git status` or `git diff` commands for post-run snapshot capture, THE Replay_Engine SHALL pass exclusion arguments (e.g., `-- ':!.repropack-replay'`) to exclude the Replay_Support_Dir from the results.
2. WHEN the Replay_Engine runs `git ls-files --others` for untracked file detection, THE Replay_Engine SHALL pass exclusion arguments to exclude the Replay_Support_Dir.
3. FOR ALL replay runs where the predicate command produces no side effects, the post-run Git snapshot SHALL report the same `changed_paths` and `untracked_paths` as the pre-run snapshot (excluding the Replay_Support_Dir).

### Requirement 20: Schema Updates for v0.3 Fields

**User Story:** As a developer, I want the JSON schemas to reflect the new fields added in v0.3, so that tooling can validate packets and receipts produced by the updated engines.

#### Acceptance Criteria

1. THE `receipt-v1.schema.json` SHALL include an optional `env_excluded_keys` array of strings.
2. THE `manifest-v1.schema.json` SHALL include an optional `redaction_report_path` string field.
3. THE schema `schema_version` strings SHALL remain `repropack.manifest.v1` and `repropack.receipt.v1` because the changes are additive and backward-compatible.
4. FOR ALL valid Manifest JSON documents produced by v0.3, serializing then deserializing through `PacketManifest` SHALL produce a semantically equivalent object (round-trip property).
5. FOR ALL valid Receipt JSON documents produced by v0.3, serializing then deserializing through `ReplayReceipt` SHALL produce a semantically equivalent object (round-trip property).

### Requirement 21: Scrub Round-Trip Integrity

**User Story:** As a developer, I want the scrubbed packet to pass all existing validation checks, so that downstream tooling treats scrubbed packets as first-class citizens.

#### Acceptance Criteria

1. FOR ALL valid Packets, scrubbing with `--public` then inspecting with `repropack inspect` SHALL succeed without errors.
2. FOR ALL valid Packets, scrubbing with `--public` then running `repropack doctor` SHALL report the packet as `blocked` (since replay fidelity is `inspect_only`) but SHALL not report integrity or schema errors.
3. THE scrubbed Manifest SHALL pass schema validation against `manifest-v1.schema.json`.
4. THE scrubbed Packet's `integrity.json` SHALL pass integrity verification via `repropack-pack::verify_integrity`.

### Requirement 22: Doctor Report for Scrubbed Packets

**User Story:** As a developer, I want `repropack doctor` to recognize scrubbed packets and report their redaction status, so that recipients of shared packets understand what was removed.

#### Acceptance Criteria

1. WHEN `repropack doctor` is run on a Packet containing a `redaction-report.json`, THE Doctor_Report SHALL include a section summarizing the redactions (count of replaced values, count of removed files).
2. THE Doctor_Report SHALL indicate that the Packet was scrubbed and is not replayable.

### Requirement 23: Config File TOML Parser

**User Story:** As a developer, I want the config file parser to handle TOML correctly, so that configuration is reliable and errors are clear.

#### Acceptance Criteria

1. THE Config_File parser SHALL accept valid TOML as defined by the TOML v1.0 specification.
2. IF the Config_File contains a syntax error, THEN THE parser SHALL return an error containing the line number and a description of the problem.
3. FOR ALL valid Config_File documents, parsing then serializing then parsing SHALL produce an equivalent configuration object (round-trip property).

### Requirement 24: CLI Smoke Tests for New Subcommands

**User Story:** As a developer, I want CLI smoke tests for all new subcommands, so that I can catch regressions in the operator surface.

#### Acceptance Criteria

1. THE CLI smoke tests SHALL verify that `repropack doctor --help` exits with code 0.
2. THE CLI smoke tests SHALL verify that `repropack explain --help` exits with code 0.
3. THE CLI smoke tests SHALL verify that `repropack shell --help` exits with code 0.
4. THE CLI smoke tests SHALL verify that `repropack scrub --help` exits with code 0.
5. THE CLI smoke tests SHALL verify that `repropack fetch --help` exits with code 0.
6. THE CLI smoke tests SHALL verify that `repropack gh --help` exits with code 0.
7. THE CLI smoke tests SHALL verify that `repropack config --help` exits with code 0.

### Requirement 25: Integration Tests for Replay Cleanliness Fixes

**User Story:** As a developer, I want integration tests that verify the replay cleanliness fixes, so that regressions in delta contamination and bundle creation are caught automatically.

#### Acceptance Criteria

1. THE Scenario_Suite SHALL include a test that captures a Packet from a root-commit-only repository and asserts the Manifest contains a valid `bundle_path`.
2. THE Scenario_Suite SHALL include a test that captures and replays a clean deterministic predicate and asserts `receipt.matched == true` with zero `warning` or `error` severity drift items.
3. THE Scenario_Suite SHALL include a test that replays a Packet and asserts that no paths under `.repropack-replay/` appear in the Receipt's capture delta drift.
4. THE Scenario_Suite SHALL include a test that replays with default settings (no `--verbose`) and asserts the Receipt contains at most one `env_excluded_summary` Drift_Item instead of per-variable items.

### Requirement 26: Integration Tests for Scrub

**User Story:** As a developer, I want integration tests for the scrub workflow, so that I can trust that scrubbed packets are safe and valid.

#### Acceptance Criteria

1. THE Scenario_Suite SHALL include a test that captures a Packet with environment variables and output files, scrubs it with `--public`, and asserts all `allowed_vars` values are `[REDACTED]` in the scrubbed Manifest.
2. THE Scenario_Suite SHALL include a test that scrubs a Packet and asserts the scrubbed Packet passes `repropack inspect` without errors.
3. THE Scenario_Suite SHALL include a test that scrubs a Packet and asserts the `redaction-report.json` contains entries for each redacted field and removed file.
4. THE Scenario_Suite SHALL include a test that scrubs a Packet and asserts `replay_fidelity` is `inspect_only` in the scrubbed Manifest.

### Requirement 27: Property Tests for Config Round-Trip

**User Story:** As a developer, I want property tests proving that config file parsing and serialization are consistent, so that I can trust the configuration layer.

#### Acceptance Criteria

1. FOR ALL valid configuration objects, serializing to TOML then parsing SHALL produce a value equal to the original.
2. FOR ALL valid configuration objects with a named profile, the resolved configuration SHALL contain all profile overrides merged over top-level defaults.

### Requirement 28: Property Tests for Scrub Invariants

**User Story:** As a developer, I want property tests proving that scrubbing preserves packet structure while removing sensitive content, so that I can trust the scrub engine.

#### Acceptance Criteria

1. FOR ALL valid Packets, scrubbing SHALL produce a Packet whose Manifest passes schema validation.
2. FOR ALL valid Packets with environment variables, scrubbing SHALL produce a Manifest where every value in `allowed_vars` is `[REDACTED]`.
3. FOR ALL valid Packets, the scrubbed Packet's `integrity.json` SHALL be internally consistent (every listed file exists and its digest matches).
4. FOR ALL valid Packets, scrubbing SHALL produce a Manifest where `replay_fidelity` is `inspect_only`.

