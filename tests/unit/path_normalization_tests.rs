//! Unit tests for path normalization
//!
//! Tests the `normalize_path()` helper function to ensure consistent path handling.

use gpui_navigator::normalize_path;

#[test]
fn test_normalize_already_normalized() {
    // Paths that are already normalized should be returned as borrowed
    assert_eq!(normalize_path("/dashboard"), "/dashboard");
    assert_eq!(normalize_path("/users/profile"), "/users/profile");
    assert_eq!(normalize_path("/"), "/");
}

#[test]
fn test_normalize_missing_leading_slash() {
    // Paths without leading slash should get one added
    assert_eq!(normalize_path("dashboard"), "/dashboard");
    assert_eq!(normalize_path("users/profile"), "/users/profile");
}

#[test]
fn test_normalize_trailing_slash() {
    // Paths with trailing slash should have it removed (except root)
    assert_eq!(normalize_path("/dashboard/"), "/dashboard");
    assert_eq!(normalize_path("/users/profile/"), "/users/profile");
}

#[test]
fn test_normalize_both_missing_and_trailing() {
    // Paths missing leading slash and having trailing slash
    assert_eq!(normalize_path("dashboard/"), "/dashboard");
    assert_eq!(normalize_path("users/profile/"), "/users/profile");
}

#[test]
fn test_normalize_empty_path() {
    // Empty path should normalize to root
    assert_eq!(normalize_path(""), "/");
}

#[test]
fn test_normalize_root_variations() {
    // Various forms of root should all normalize to "/"
    assert_eq!(normalize_path("/"), "/");
    assert_eq!(normalize_path("//"), "/");
    assert_eq!(normalize_path("///"), "/");
}

#[test]
fn test_normalize_with_parameters() {
    // Paths with parameter segments should be normalized correctly
    assert_eq!(normalize_path("/users/:id"), "/users/:id");
    assert_eq!(normalize_path("users/:id"), "/users/:id");
    assert_eq!(normalize_path("/users/:id/"), "/users/:id");
}
