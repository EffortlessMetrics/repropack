# Product: ReproPack

ReproPack is a commit-aware failure packet generator for ordinary Git repositories.

It captures a repo predicate (an arbitrary command run against a specific code state) and emits a portable `.rpk` packet that humans, CI jobs, or agents can inspect and replay.

## Core flows

- `capture` — runs a command, records repo state + execution evidence, emits a packet
- `inspect` — reads a packet without replay; renders summary, JSON, or tree
- `replay` — materializes repo state from the packet and reruns the command on the host
- `unpack` — extracts a `.rpk` archive to a directory
- `emit` — generates CI integration snippets (currently GitHub Actions)

## Key contracts

- The packet spec (`PACKET-SPEC.md`) is the product. The archive is just transport.
- `manifest.json` is authoritative inside every packet.
- Replay is honest: missing context becomes structured drift, never a false success.
- Omissions and redactions are first-class, not errors.
- Schema versions are explicit (`repropack.manifest.v1`, `repropack.receipt.v1`). Breaking changes require a version bump.

## Current scope (v0.1)

Host replay only. No syscall tracing, container replay, plugin system, service backend, or packet signing. These are deliberate non-goals for the first release.
