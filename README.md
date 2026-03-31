# ReproPack

ReproPack is a **commit-aware failure packet generator** for ordinary repositories.

It captures a repo predicate at a specific code state and emits a portable packet that another human, CI job, or agent can inspect and replay. The packet is narrower than full-system reproducibility and broader than a test-run artifact:

- **arbitrary repo command in**
- **portable packet out**
- **truthful replay receipt back**

## Current scope

This repo is a v0.1 scaffold with a working shape for:

- `repropack capture -- <cmd> <args...>`
- `repropack inspect <packet.rpk>`
- `repropack replay <packet.rpk>`
- `repropack unpack <packet.rpk> --out <dir>`
- `repropack emit github-actions`

## Product principles

ReproPack is designed as a **contract-first CLI**:

- the packet spec is the product
- the archive is transport
- replay is honest about drift
- omissions and redactions are first-class
- Git is the repo substrate
- GitHub artifacts are transport, not identity

## What v0.1 does

### Capture

`capture` records:

- command argv and working directory
- exit status, duration, stdout, stderr
- commit SHA, ref, changed paths, dirty worktree state
- optional Git bundle and worktree patch
- selected inputs and outputs
- platform fingerprint and selected environment variables
- tool versions for common binaries and the invoked command
- Markdown and HTML summaries

### Inspect

`inspect` renders the packet without reconstructing the original machine. It can print:

- the human summary
- the raw manifest JSON
- a packet tree view

### Replay

`replay` is **host replay**, not fake hermetic replay. It will:

1. materialize repo state from the packet
2. restore overlays and selected environment variables
3. compare tool versions
4. rerun the command when replay is allowed
5. emit a replay receipt with drift notes

## Quick start

```bash
cargo run -p repropack-cli -- capture --name ci-red -- cargo test
cargo run -p repropack-cli -- inspect ./ci-red-<packet-id>.rpk
cargo run -p repropack-cli -- replay ./ci-red-<packet-id>.rpk --into /tmp/repropack-run
```

### Capture a dirty local failure

```bash
cargo run -p repropack-cli -- \
  capture \
  --name local-red \
  --include reports/** \
  --output target/**/junit.xml \
  --git-bundle auto \
  -- cargo nextest run
```

## Workspace map

| Crate | Role |
|---|---|
| `repropack-model` | manifest and receipt types |
| `repropack-pack` | pack/unpack, hashing, packet materialization |
| `repropack-git` | repo inspection, diffs, bundles |
| `repropack-render` | Markdown and HTML summaries |
| `repropack-capture` | capture orchestration |
| `repropack-replay` | replay orchestration |
| `repropack-cli` | operator surface |
| `xtask` | repo rituals and scenario atlas generation |

## Repo doctrine

This workspace follows a delegation-aware, artifact-first Rust shape:

- small crates around real seams
- truth in the manifest and receipt types
- Git and process execution at the edges
- CLI behavior backed by durable artifacts
- docs, scenario atlas, and command surface treated as architecture

See:

- [`PACKET-SPEC.md`](PACKET-SPEC.md)
- [`REPLAY.md`](REPLAY.md)
- [`SECURITY.md`](SECURITY.md)
- [`TESTING.md`](TESTING.md)
- [`AGENTS.md`](AGENTS.md)
- [`docs/architecture.md`](docs/architecture.md)
- [`docs/scenario_index.md`](docs/scenario_index.md)

## Known limits in v0.1

- host replay only
- no syscall or file-access tracing
- no container replay
- no plugin system
- no service backend
- no packet signing yet
- bundle creation is simple, not minimal

Those are deliberate non-goals for the first cut.
