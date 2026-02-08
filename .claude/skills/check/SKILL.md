---
name: check
description: Run full Rust validation pipeline (build, clippy, tests, fmt)
allowed-tools: Bash
---

Run the full validation pipeline for the project. Execute each step sequentially and report results:

1. **Build check**: `cargo check --all-features`
2. **Clippy**: `cargo clippy --all-targets --all-features`
3. **Tests**: `cargo test --all-features`
4. **Format check**: `cargo fmt --check`

If any step fails, stop and report the error with details on how to fix it.
At the end, provide a summary: which steps passed and which failed.
