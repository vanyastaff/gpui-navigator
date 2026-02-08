//! Unit tests for parameter name extraction
//!
//! Tests the `extract_param_name()` helper function to ensure proper parameter parsing.

use gpui_navigator::extract_param_name;

#[test]
fn test_extract_simple_parameter() {
    // Simple parameter with colon prefix
    assert_eq!(extract_param_name(":id"), "id");
    assert_eq!(extract_param_name(":user_id"), "user_id");
    assert_eq!(extract_param_name(":name"), "name");
}

#[test]
fn test_extract_parameter_with_constraint() {
    // Parameters with type constraints should strip the constraint
    assert_eq!(extract_param_name(":id<i32>"), "id");
    assert_eq!(extract_param_name(":user_id<uuid>"), "user_id");
    assert_eq!(extract_param_name(":count<u32>"), "count");
}

#[test]
fn test_extract_parameter_with_complex_constraint() {
    // Parameters with more complex constraint syntax
    assert_eq!(extract_param_name(":id<regex:[0-9]+>"), "id");
    assert_eq!(extract_param_name(":slug<path>"), "slug");
}

#[test]
fn test_extract_without_colon() {
    // Edge case: segment without colon (shouldn't happen in normal use)
    assert_eq!(extract_param_name("id"), "id");
    assert_eq!(extract_param_name("id<i32>"), "id");
}

#[test]
fn test_extract_empty_and_edge_cases() {
    // Edge cases
    assert_eq!(extract_param_name(":"), "");
    assert_eq!(extract_param_name(":<i32>"), "");
}
