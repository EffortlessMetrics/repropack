// Feature: repropack-v02-alpha, Property 10: Per-file size cap truncates and records omission
// Feature: repropack-v02-alpha, Property 11: Total packet size cap stops capture and records omission

use proptest::prelude::*;
use repropack_capture::{apply_file_size_cap, would_exceed_packet_cap};

proptest! {
    /// **Validates: Requirements 11.1, 11.2**
    ///
    /// Property 10: For any artifact whose size exceeds the configured per-file
    /// size cap, the capture engine shall write at most `max_file_bytes` bytes
    /// and the truncation is correctly applied.
    #[test]
    fn per_file_size_cap_truncates(
        data in prop::collection::vec(any::<u8>(), 1..2048),
        max_file_bytes in 1u64..1024,
    ) {
        let result = apply_file_size_cap(&data, max_file_bytes);
        let original_size = data.len() as u64;

        if original_size > max_file_bytes {
            // File was truncated: result must be exactly max_file_bytes long
            prop_assert_eq!(
                result.len() as u64,
                max_file_bytes,
                "truncated file should be exactly max_file_bytes"
            );
            // Truncated content must be a prefix of the original
            prop_assert_eq!(
                &result[..],
                &data[..max_file_bytes as usize],
                "truncated content must be a prefix of original"
            );
        } else {
            // File fits within cap: result must equal original
            prop_assert_eq!(
                result.len(),
                data.len(),
                "non-truncated file should keep original size"
            );
            prop_assert_eq!(
                &result[..],
                &data[..],
                "non-truncated file should keep original content"
            );
        }
    }

    /// **Validates: Requirements 11.3, 11.4**
    ///
    /// Property 11: For any collection of artifacts whose cumulative size would
    /// exceed the configured total size cap, the capture engine shall stop
    /// capturing additional artifacts.
    #[test]
    fn total_packet_size_cap_stops_capture(
        cumulative_size in 0u64..10_000,
        file_size in 1u64..5_000,
        max_packet_bytes in 1u64..15_000,
    ) {
        let exceeds = would_exceed_packet_cap(cumulative_size, file_size, max_packet_bytes);

        if cumulative_size + file_size > max_packet_bytes {
            prop_assert!(
                exceeds,
                "should detect packet size exceeded when cumulative {} + file {} > cap {}",
                cumulative_size,
                file_size,
                max_packet_bytes
            );
        } else {
            prop_assert!(
                !exceeds,
                "should not flag exceeded when cumulative {} + file {} <= cap {}",
                cumulative_size,
                file_size,
                max_packet_bytes
            );
        }
    }
}
