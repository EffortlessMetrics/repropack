# Agent guidance

This repo is designed so that trusted change can be delegated without losing control.

## Crate map

- `repropack-model` → packet and receipt truth
- `repropack-pack` → archive transport and integrity
- `repropack-git` → repo inspection and materialization
- `repropack-render` → human-facing summaries
- `repropack-capture` → capture flow orchestration
- `repropack-replay` → replay flow orchestration
- `repropack-cli` → operator surface
- `xtask` → repo rituals

## Dependency direction

Keep the direction simple:

- `model` has no dependency on application crates
- `pack`, `git`, `render` depend on `model`
- `capture` and `replay` depend on `model` plus edge crates
- `cli` depends on all orchestration crates
- `xtask` is repo-local and operational only

## Rules

1. Start from the packet contract, not from helper functions.
2. Prefer adding fields to the manifest or receipt over inventing side channels.
3. If a behavior changes operator-visible output, update snapshots and docs.
4. If a change alters replay safety or fidelity, update `PACKET-SPEC.md`, `REPLAY.md`, and the scenario atlas.
5. Do not add hidden capture of secrets, network state, or external service behavior.
6. Keep replay honest. Missing context should become structured drift, not a false success claim.

## Commands

```bash
cargo xtask ci-fast
cargo xtask ci-full
cargo xtask scenario-index
cargo xtask docs-check
cargo xtask release-check
```

## When to stop and escalate

Stop and escalate when a requested change would require:

- silent secret capture
- unsafe auto-replay of mutating commands
- redefining `exact` replay without new evidence
- backwards-incompatible schema changes without a version bump
- adding a backend or plugin model without clear product need
