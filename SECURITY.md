# Security notes

ReproPack handles executable commands and environment data. The secure posture is conservative.

## Packet input is untrusted

Treat incoming packets as untrusted input:

- validate paths when unpacking
- reject path traversal and absolute archive members
- do not auto-run replay from unknown packets without inspection

The unpacker in this repo enforces basic path traversal protection.

## Environment capture is allow-list first

ReproPack does **not** dump the full environment by default.

### Default allow patterns

- `CI`
- `GITHUB_*`
- `RUSTUP_TOOLCHAIN`
- `CARGO_*`

### Default deny patterns

- `*TOKEN*`
- `*SECRET*`
- `*PASSWORD*`
- `AWS_*`
- `GH_TOKEN`
- `GITHUB_TOKEN`
- `CARGO_REGISTRY_TOKEN`

Denied keys are recorded as redactions, not silently ignored.

## Replay policy matters

Replaying arbitrary commands is risky.

Use replay policies:

- `safe`
- `confirm`
- `disabled`

If the original command is not a verification-style command, prefer `confirm` or `disabled`.

## Known gaps in v0.1

- no packet signing
- no tamper-evident manifest envelope
- no size caps for captured logs or files
- no sandbox for replay
- no syscall tracing to discover hidden inputs

These are known follow-on items, not hidden assumptions.
