// Feature: repropack-v02-alpha, Property 1: Capture delta is correct set difference

use proptest::prelude::*;
use repropack_git::compute_capture_delta;
use repropack_model::GitSnapshot;
use std::collections::BTreeSet;

/// Generate an arbitrary `GitSnapshot` with random path sets.
fn arb_git_snapshot() -> impl Strategy<Value = GitSnapshot> {
    let arb_paths = prop::collection::vec("[a-z]{1,3}/[a-z]{1,5}\\.[a-z]{1,3}", 0..8);
    (
        prop::option::of("[0-9a-f]{40}"),
        any::<bool>(),
        arb_paths.clone(),
        arb_paths,
    )
        .prop_map(
            |(commit_sha, is_dirty, changed_paths, untracked_paths)| GitSnapshot {
                commit_sha,
                is_dirty,
                changed_paths,
                untracked_paths,
                worktree_patch_path: None,
            },
        )
}

proptest! {
    /// **Validates: Requirements 3.1**
    ///
    /// For any two GitSnapshot values, compute_capture_delta produces:
    /// - newly_dirty_paths    = post.changed_paths − pre.changed_paths
    /// - newly_untracked_paths = post.untracked_paths − pre.untracked_paths
    /// - newly_modified_paths  = intersection(pre.changed_paths, post.changed_paths)
    #[test]
    fn capture_delta_is_correct_set_difference(
        pre in arb_git_snapshot(),
        post in arb_git_snapshot(),
    ) {
        let delta = compute_capture_delta(&pre, &post);

        let pre_changed: BTreeSet<&str> =
            pre.changed_paths.iter().map(String::as_str).collect();
        let post_changed: BTreeSet<&str> =
            post.changed_paths.iter().map(String::as_str).collect();
        let pre_untracked: BTreeSet<&str> =
            pre.untracked_paths.iter().map(String::as_str).collect();
        let post_untracked: BTreeSet<&str> =
            post.untracked_paths.iter().map(String::as_str).collect();

        // newly_dirty_paths = post.changed − pre.changed
        let expected_dirty: BTreeSet<&str> =
            post_changed.difference(&pre_changed).copied().collect();
        let actual_dirty: BTreeSet<&str> =
            delta.newly_dirty_paths.iter().map(String::as_str).collect();
        prop_assert_eq!(&expected_dirty, &actual_dirty,
            "newly_dirty_paths mismatch");

        // newly_modified_paths = intersection(pre.changed, post.changed)
        let expected_modified: BTreeSet<&str> =
            post_changed.intersection(&pre_changed).copied().collect();
        let actual_modified: BTreeSet<&str> =
            delta.newly_modified_paths.iter().map(String::as_str).collect();
        prop_assert_eq!(&expected_modified, &actual_modified,
            "newly_modified_paths mismatch");

        // newly_untracked_paths = post.untracked − pre.untracked
        let expected_untracked: BTreeSet<&str> =
            post_untracked.difference(&pre_untracked).copied().collect();
        let actual_untracked: BTreeSet<&str> =
            delta.newly_untracked_paths.iter().map(String::as_str).collect();
        prop_assert_eq!(&expected_untracked, &actual_untracked,
            "newly_untracked_paths mismatch");
    }
}
