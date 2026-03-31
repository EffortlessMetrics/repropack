# Packet specification

ReproPack treats the packet as a **directory contract with an archive wrapper**.

The packet format is currently a gzip-compressed tar archive with a `.rpk` extension. The durable contract is the directory layout and the `manifest.json` schema, not the compression choice.

## Packet goals

A packet must answer five questions:

1. What was run?
2. What code state was it run against?
3. What environment shaped the result?
4. What evidence came out?
5. How replayable is this packet?

## Layout

```text
packet.rpk
├── manifest.json
├── summary.md
├── summary.html
├── git/
│   ├── commit.json
│   ├── changed-paths.txt
│   ├── diff.patch
│   ├── worktree.patch
│   └── repo.bundle
├── exec/
│   ├── argv.json
│   ├── exit.json
│   ├── stdout.log
│   └── stderr.log
├── env/
│   ├── platform.json
│   ├── keys.json
│   ├── allowed-values.json
│   └── tool-versions.json
├── inputs/
│   ├── index.json
│   └── files/...
└── outputs/
    ├── index.json
    └── files/...
```

## The manifest

`manifest.json` is authoritative.

### Top-level fields

- `schema_version`
- `packet_id`
- `packet_name`
- `created_at`
- `capture_level`
- `replay_fidelity`
- `replay_policy`
- `command`
- `execution`
- `git`
- `environment`
- `inputs`
- `outputs`
- `packet_files`
- `omissions`
- `notes`

## Capture levels

### `metadata`

Packet includes command, execution, environment, and summaries.

### `repo`

Packet includes metadata plus Git state and repo-oriented evidence.

### `inputs`

Packet includes repo state plus explicit input and output captures.

## Replay fidelity

### `exact`

Packet contains enough information for a strong host replay claim.

Typical conditions:

- repo bundle present
- no critical omissions
- no replay-disabled policy

### `approximate`

Packet can usually be replayed, but something important was omitted or redacted.

Typical causes:

- selected environment variables were redacted
- bundle generation failed
- external inputs were captured for inspection but cannot be restored into the repo

### `inspect_only`

Packet is for inspection only.

Typical causes:

- replay policy disabled
- repo state missing
- command intentionally marked as non-replayable

## Replay policy

### `safe`

Replay can run without extra confirmation.

### `confirm`

Replay may mutate state and requires explicit confirmation or `--force`.

### `disabled`

Replay is blocked. The packet is inspection-only.

## Omissions

Omissions are explicit. They are used for:

- bundle creation failures
- missing glob matches
- redacted environment variables
- external inputs that cannot be restored into the repo
- replay skips and safety blocks

An omission is not a crash. It is a first-class statement that the packet is incomplete in a specific, named way.

## Packet files

`packet_files` records every file present in the packet except `manifest.json` itself.

That exclusion is intentional. The manifest is the source of truth and is not self-hashed in v0.1. Later versions can add a detached integrity envelope if needed.

## Determinism

The packer enforces deterministic ordering:

- sorted relative paths
- normalized archive timestamps
- digests for each recorded file
- stable JSON serialization

## Compatibility

The schema uses explicit version strings:

- `repropack.manifest.v1`
- `repropack.receipt.v1`

A breaking manifest change should create `v2`, not silently reinterpret `v1`.
