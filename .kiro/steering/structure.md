# Project Structure

## Workspace layout

```
crates/
  repropack-model/    — Manifest and receipt types (no app dependencies)
  repropack-pack/     — Archive pack/unpack, hashing, materialization
  repropack-git/      — Git inspection, diffs, bundles (shells out to git CLI)
  repropack-render/   — Markdown and HTML summary rendering
  repropack-capture/  — Capture flow orchestration
  repropack-replay/   — Replay flow orchestration
  repropack-cli/      — CLI binary (operator surface)
  xtask/              — Repo rituals (CI, docs-check, scenario-index, etc.)
docs/                 — Architecture docs and generated scenario atlas
schema/               — JSON schemas for manifest and receipt
examples/             — Sample manifest and receipt JSON, GitHub Actions workflow
```

## Dependency direction

```
model          (leaf — no internal deps)
  ↑
pack, git, render   (depend on model)
  ↑
capture, replay     (depend on model + edge crates)
  ↑
cli                 (depends on all orchestration crates)

xtask               (standalone, repo-local only)
```

This is enforced by convention. Do not introduce upward or circular dependencies.

## Key files

| File | Role |
|---|---|
| `PACKET-SPEC.md` | Packet format contract (directory layout + manifest schema) |
| `REPLAY.md` | Replay model, safety rules, drift semantics |
| `TESTING.md` | Testing strategy and tier map |
| `AGENTS.md` | Agent guidance, escalation rules |
| `SECURITY.md` | Security posture |
| `docs/architecture.md` | Architecture overview (capture/inspect/replay flows) |
| `docs/scenario_index.md` | Generated scenario atlas (via `cargo xtask scenario-index`) |

## Rules for changes

- Start from the packet contract (`repropack-model`), not from helpers.
- Prefer adding fields to manifest/receipt over inventing side channels.
- If a change alters operator-visible output, update snapshots and docs.
- If a change alters replay safety or fidelity, update `PACKET-SPEC.md`, `REPLAY.md`, and the scenario atlas.
- Do not add hidden capture of secrets, network state, or external service behavior.
