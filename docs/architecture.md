# Architecture

ReproPack is shaped around three flows:

- **capture**
- **inspect**
- **replay**

The architecture is intentionally small and explicit.

## Capture flow

```text
command + repo state + selected environment
    ↓
execution record
    ↓
git capture
    ↓
input/output selection
    ↓
manifest assembly
    ↓
summary render
    ↓
packet archive
```

## Inspect flow

```text
packet archive or packet directory
    ↓
materialize
    ↓
read manifest
    ↓
render summary / JSON / tree
```

## Replay flow

```text
packet
    ↓
materialize
    ↓
policy check
    ↓
clone from bundle
    ↓
apply patch and overlays
    ↓
compare tool versions
    ↓
rerun command
    ↓
receipt
```

## Why this repo is microcrated

The seams are real:

- manifest and receipt types are stable truth
- archive transport has separate failure modes
- Git interactions are edge-heavy and deserve isolation
- rendering is artifact-facing
- capture and replay each own their own orchestration semantics

This keeps intent legible for maintainers and agents.

## Design boundary

The first version does **not** attempt to be a full-system reproducibility framework.

It is a repo-ops compiler:

- hard repo operation in
- deterministic packet and receipt out
