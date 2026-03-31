//! CLI smoke tests (Tasks 18.1–18.4)
//!
//! Each test invokes the `repropack` binary via `std::process::Command`
//! and asserts on exit code and output.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Locate the repropack binary built by cargo.
fn repropack_bin() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("parent of test binary")
        .parent()
        .expect("parent of deps dir")
        .to_path_buf();
    path.push("repropack");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    assert!(
        path.exists(),
        "repropack binary not found at {}",
        path.display()
    );
    path
}

// ── 18.1: `repropack --help` exits 0 and contains expected subcommands ──
// Validates: Requirement 16.1

#[test]
fn help_exits_zero_and_lists_subcommands() {
    let output = Command::new(repropack_bin())
        .arg("--help")
        .output()
        .expect("failed to run repropack --help");

    assert!(
        output.status.success(),
        "repropack --help should exit 0, got {:?}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for subcommand in &["capture", "inspect", "replay", "unpack"] {
        assert!(
            stdout.contains(subcommand),
            "repropack --help output should contain '{subcommand}', got:\n{stdout}"
        );
    }
}

// ── 18.2: `repropack capture --help` exits 0 ───────────────────────
// Validates: Requirement 16.2

#[test]
fn capture_help_exits_zero() {
    let output = Command::new(repropack_bin())
        .args(["capture", "--help"])
        .output()
        .expect("failed to run repropack capture --help");

    assert!(
        output.status.success(),
        "repropack capture --help should exit 0, got {:?}",
        output.status
    );
}

// ── 18.3: `repropack inspect` with valid packet exits 0 and shows packet ID ─
// Validates: Requirement 16.3

#[test]
fn inspect_valid_packet_exits_zero_and_shows_packet_id() {
    let tmp = tempfile::tempdir().unwrap();
    let packet_dir = tmp.path().join("test-packet");
    fs::create_dir_all(&packet_dir).unwrap();

    let packet_id = "deadbeef-1234-5678-9abc-def012345678";

    // Build a minimal valid manifest.json
    let manifest = serde_json::json!({
        "schema_version": "repropack.manifest.v1",
        "packet_id": packet_id,
        "created_at": "2026-01-15T12:00:00Z",
        "capture_level": "metadata",
        "replay_fidelity": "exact",
        "replay_policy": "safe",
        "command": {
            "program": "echo",
            "args": ["hello"],
            "display": "echo hello",
            "cwd": "/tmp"
        },
        "execution": {
            "started_at": "2026-01-15T12:00:00Z",
            "finished_at": "2026-01-15T12:00:01Z",
            "duration_ms": 1000,
            "success": false
        },
        "environment": {
            "platform": {
                "family": "unix",
                "os": "linux",
                "arch": "x86_64"
            },
            "allowed_vars": {},
            "redacted_keys": [],
            "tool_versions": {}
        },
        "inputs": [],
        "outputs": [],
        "packet_files": [],
        "omissions": [],
        "notes": []
    });

    fs::write(
        packet_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let output = Command::new(repropack_bin())
        .args(["inspect", packet_dir.to_str().unwrap()])
        .output()
        .expect("failed to run repropack inspect");

    assert!(
        output.status.success(),
        "repropack inspect should exit 0, got {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(packet_id),
        "inspect output should contain packet ID '{packet_id}', got:\n{stdout}"
    );
}

// ── 18.4: `repropack capture` with no command exits non-zero ────────
// Validates: Requirement 16.4

#[test]
fn capture_no_command_exits_nonzero() {
    let output = Command::new(repropack_bin())
        .args(["capture"])
        .output()
        .expect("failed to run repropack capture");

    assert!(
        !output.status.success(),
        "repropack capture with no command should exit non-zero, got {:?}",
        output.status
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    // clap should produce an error message about missing required argument
    assert!(
        !stderr.is_empty(),
        "repropack capture with no command should produce error output"
    );
}
