---
name: fix-warnings
description: Find and fix all Rust compiler and clippy warnings
allowed-tools: Bash, Read, Edit, Grep
---

Find and fix all warnings in the project:

1. Run `cargo clippy --all-targets --all-features 2>&1` and capture output
2. Parse all warnings (unused variables, dead code, unused imports, etc.)
3. For each warning:
   - Read the affected file
   - Apply the appropriate fix (prefix with `_`, remove unused code, etc.)
   - Verify the fix doesn't break functionality
4. Run clippy again to confirm all warnings are resolved
5. Run `cargo test --all-features` to ensure nothing broke

Fix conservatively:
- Unused variables: prefix with `_`
- Unused imports: remove them
- Dead code: add `#[allow(dead_code)]` only if the code is needed for future use, otherwise remove it
- Unused functions in test helpers: add `#[allow(dead_code)]` since they may be used by future tests
