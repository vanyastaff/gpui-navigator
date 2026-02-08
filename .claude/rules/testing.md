---
paths:
  - "tests/**/*.rs"
---

# Testing Rules

## Test Organization

- Unit tests: `tests/unit/<module>.rs` — mirrors `src/<module>.rs`
- Integration tests: `tests/integration/<feature>.rs`
- Tests requiring GPUI context use `#[gpui::test]` attribute
- Common test helpers go in `tests/common/mod.rs`

## Test Naming

- Format: `test_<what>_<scenario>` or `test_<what>_<expected_result>`
- Examples: `test_nested_route_matching_with_params`, `test_cache_eviction_on_capacity`

## Test Requirements

- Every new public function needs at least one test
- Test both happy path and edge cases
- Use descriptive assertion messages: `assert_eq!(result, expected, "should match parent route")`
- `.unwrap()` is fine in tests — panics give clear failure messages

## Running Tests

- Always run with all features: `cargo test --all-features`
- For focused testing: `cargo test --all-features -- test_name`
- Run clippy after changes: `cargo clippy --all-targets --all-features`
