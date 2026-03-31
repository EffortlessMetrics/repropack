# Testing strategy

This repo treats testing as architecture.

The stack is layered rather than substitutable:

- **scenario tests** index behavior
- **property tests** generalize invariants
- **mutation tests** audit proof depth
- **snapshot tests** stabilize artifacts
- **integration tests** admit reality
- **smoke tests** validate the operator path

## Tier map

### Tier 0 — model and schema

Crates:

- `repropack-model`

Expected proof:

- serde round-trip tests
- manifest and receipt snapshots
- schema compatibility checks

### Tier 1 — repo and packet translation edges

Crates:

- `repropack-pack`
- `repropack-git`

Expected proof:

- temp-repo integration tests
- archive determinism tests
- malformed packet tests
- packet tree and digest tests

### Tier 2 — orchestration

Crates:

- `repropack-capture`
- `repropack-replay`

Expected proof:

- scenario-heavy tests
- exit-code preservation tests
- omission and fidelity classification tests
- replay drift tests

### Tier 3 — operator surface

Crates:

- `repropack-cli`

Expected proof:

- `--help` coverage
- exit code tests
- summary snapshots
- temp workspace smoke tests

### Tier 4 — repo proof surface

Commands:

- `cargo xtask ci-fast`
- `cargo xtask ci-full`
- `cargo xtask scenario-index`
- `cargo xtask docs-check`
- `cargo xtask release-check`

## Current status

This scaffold ships the structure and the contract first. Scenario and mutation coverage are sketched through the scenario atlas and xtask surface, but the full test matrix still needs to be fleshed out once the Rust toolchain is available in the target environment.
