---
name: test
description: Run tests with optional filter and detailed output
allowed-tools: Bash
argument-hint: "[test_name_filter]"
---

Run project tests with detailed output.

If `$ARGUMENTS` is provided, use it as a test name filter:
```
cargo test --all-features -- $ARGUMENTS --nocapture
```

If no arguments, run all tests:
```
cargo test --all-features
```

After running, summarize:
- Total tests run
- Passed / Failed / Ignored counts
- If any failed — show the failure details and suggest fixes
