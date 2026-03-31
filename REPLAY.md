# Replay model

ReproPack v0.1 supports **host replay**.

That means replay is honest about the host it is running on. It does not claim to have captured the whole machine.

## Replay flow

1. materialize the packet
2. validate replay policy
3. create a fresh work directory
4. clone from the packet bundle if present
5. check out the recorded commit
6. apply the worktree patch if present
7. restore explicit input overlays
8. overlay allowed environment variables
9. probe tool versions and compare them to the packet
10. rerun the command
11. emit a replay receipt

## Output

Replay writes:

- `receipt.json`
- `receipt.md`
- `stdout.log`
- `stderr.log`

The default location is:

```text
<workdir>/.repropack-replay/
```

## Safety

Replay never writes into the current worktree by default.

Replay respects the packet policy:

- `safe` → runs normally
- `confirm` → requires `--force`
- `disabled` → blocked

## Drift

The replay receipt records drift as structured items, for example:

- tool version mismatch
- missing bundle
- mismatched exit code
- worktree patch application failure
- blocked replay policy

A replay can be operationally successful while still reporting drift.

## Exit behavior

- `capture` preserves the original command exit code
- `replay` preserves the observed replay command exit code when it runs
- blocked or `--no-run` replay returns zero and relies on the receipt for status

## What replay is not

ReproPack does **not** yet do:

- full dependency capture
- network capture
- syscall tracing
- container replay
- service mocking

That is intentional. The point of the first release is **portable repo reproduction**, not universal machine reconstruction.
