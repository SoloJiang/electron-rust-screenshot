//! End-to-end smoke test: launches the validator binary against
//! validator-specs/smoke_cancel.toml and asserts exit code 0.
//!
//! Marked #[ignore] because it requires a built dist/ via `npm run build`;
//! CI explicitly opts in via `cargo test --test smoke -- --ignored`.

use std::process::Command;

#[test]
#[ignore]
fn smoke_cancel_passes() {
    let status = Command::new(env!("CARGO_BIN_EXE_validator"))
        .args([
            "--out",
            "validator-output",
            "validator-specs/smoke_cancel.toml",
        ])
        .status()
        .expect("validator failed to start");
    assert!(status.success(), "validator exited non-zero");
}
