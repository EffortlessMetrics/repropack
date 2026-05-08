# Coverage

Codecov coverage is Rust execution-surface evidence for the `repropack` repository.

It answers:
> Did tests execute this Rust surface?

It does not answer:
- whether a packet is correct,
- whether capture preserves the right inputs and outputs,
- whether replay is faithful,
- whether Git bundle or patch materialization is complete,
- whether overlay restoration is correct,
- whether redaction is safe,
- whether host drift detection is correct,
- whether artifact inventories are complete,
- whether scenario coverage is adequate,
- whether release readiness is proven.

Those are separate proof lanes.

## Runs

The Coverage workflow runs on:
- push to `main`,
- `workflow_dispatch`,
- PRs labeled `coverage`, `full-ci`, or `ci:full`.

## Artifacts and receipts

Codecov comments are disabled. Durable receipts are:
- `coverage.json`,
- `coverage.txt`,
- `lcov.info`,
- the GitHub Actions coverage artifact,
- the Codecov dashboard.
