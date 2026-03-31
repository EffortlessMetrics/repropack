# Scenario atlas

This file is the first scenario atlas for the repo.

| Scenario | Problem | Crates | Artifact | Notes |
|---|---|---|---|---|
| capture clean commit failure | preserve a red predicate with commit state | `capture`, `git`, `pack`, `render` | `manifest.json`, `summary.md` | basic CI path |
| capture dirty worktree failure | preserve local deltas honestly | `capture`, `git` | `worktree.patch` | tracked changes only unless explicitly included |
| inspect packet without replay | answer what ran and what happened | `pack`, `render`, `cli` | `summary.md`, JSON | offline operator path |
| replay from bundle | reconstruct repo and rerun | `replay`, `git` | `receipt.json` | host replay only |
| blocked replay policy | avoid unsafe reruns | `replay`, `cli` | `receipt.json` | `confirm` needs `--force`; `disabled` blocks |
| redacted environment capture | preserve safety while keeping evidence | `capture`, `render` | omissions + summary | redactions are explicit |
| missing glob matches | make incomplete packets honest | `capture` | omissions | no hidden no-ops |

The atlas should grow alongside behavior.
