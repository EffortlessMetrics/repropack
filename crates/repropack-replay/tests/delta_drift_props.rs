// Feature: repropack-v02-alpha, Property 17: Capture delta drift comparison

use proptest::prelude::*;
use repropack_model::CaptureDelta;
use repropack_replay::compare_capture_deltas;

/// Generate an arbitrary CaptureDelta with random path sets.
fn arb_capture_delta() -> impl Strategy<Value = CaptureDelta> {
    let arb_paths = prop::collection::vec("[a-z]{1,3}/[a-z]{1,5}\\.[a-z]{1,3}", 0..6);
    (arb_paths.clone(), arb_paths.clone(), arb_paths).prop_map(
        |(newly_dirty_paths, newly_modified_paths, newly_untracked_paths)| CaptureDelta {
            newly_dirty_paths,
            newly_modified_paths,
            newly_untracked_paths,
        },
    )
}

proptest! {
    /// **Validates: Requirements 18.2**
    ///
    /// Property 17: For any two CaptureDelta values where at least one field
    /// differs, compare_capture_deltas shall produce a DriftItem with subject
    /// "capture_delta" and severity "warning".
    #[test]
    fn capture_delta_drift_when_different(
        manifest_delta in arb_capture_delta(),
        replay_delta in arb_capture_delta(),
    ) {
        let result = compare_capture_deltas(&manifest_delta, &replay_delta);

        if manifest_delta != replay_delta {
            // Must produce a drift item
            let item = result.as_ref();
            prop_assert!(
                item.is_some(),
                "must produce DriftItem when deltas differ"
            );
            let item = item.unwrap();
            prop_assert_eq!(
                &item.subject,
                "capture_delta",
                "drift subject must be capture_delta"
            );
            prop_assert!(
                matches!(item.severity, repropack_model::Severity::Warning),
                "drift severity must be warning"
            );
        } else {
            // Equal deltas must produce no drift
            prop_assert!(
                result.is_none(),
                "must not produce DriftItem when deltas are equal"
            );
        }
    }

    /// **Validates: Requirements 18.2**
    ///
    /// Property 17 (equal case): For any CaptureDelta compared against itself,
    /// compare_capture_deltas shall produce None.
    #[test]
    fn capture_delta_no_drift_when_equal(
        delta in arb_capture_delta(),
    ) {
        let result = compare_capture_deltas(&delta, &delta);
        prop_assert!(
            result.is_none(),
            "must not produce DriftItem when deltas are identical"
        );
    }
}
