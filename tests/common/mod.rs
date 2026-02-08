//! Test utilities for nested routing tests
//!
//! Provides fixtures, helpers, and assertion utilities for unit and integration tests.

#![allow(dead_code)]

use gpui::*;
use gpui_navigator::*;

/// Create a simple test route with given path and name
pub fn create_test_route(path: &str, name: Option<&str>) -> Route {
    let mut route = Route::view(path, || div().child("Test Page").into_any_element());

    if let Some(n) = name {
        route = route.name(n);
    }

    route
}

/// Create a route with children for testing nested routing
pub fn create_parent_with_children(parent_path: &str, child_paths: Vec<&str>) -> Route {
    let children: Vec<std::sync::Arc<Route>> = child_paths
        .into_iter()
        .map(|path| std::sync::Arc::new(create_test_route(path, None)))
        .collect();

    Route::view(parent_path, || {
        div().child("Parent Page").into_any_element()
    })
    .children(children)
}

/// Create a route with parameter
pub fn create_param_route(path: &str) -> Route {
    Route::new(path, |_, _, _| div().child("Param Page").into_any_element())
}

/// Create a route tree for deep nesting tests (3 levels)
pub fn create_deep_nested_tree() -> Route {
    Route::view("/level1", || div().child("Level 1").into_any_element()).children(vec![
        std::sync::Arc::new(
            Route::view("level2", || div().child("Level 2").into_any_element()).children(vec![
                std::sync::Arc::new(Route::view("level3", || {
                    div().child("Level 3").into_any_element()
                })),
            ]),
        ),
    ])
}

/// Assert that route parameters contain expected key-value pair
pub fn assert_param_equals(params: &RouteParams, key: &str, expected: &str) {
    let value = params.get(key);
    assert!(
        value.is_some(),
        "Parameter '{}' not found in RouteParams",
        key
    );
    assert_eq!(
        value.unwrap(),
        expected,
        "Parameter '{}' has wrong value",
        key
    );
}

/// Assert that route parameters do NOT contain a key
pub fn assert_param_not_present(params: &RouteParams, key: &str) {
    assert!(
        params.get(key).is_none(),
        "Parameter '{}' should not be present",
        key
    );
}

/// Create empty RouteParams for testing
pub fn empty_params() -> RouteParams {
    RouteParams::new()
}

/// Create RouteParams with single key-value pair
pub fn params_with(key: &str, value: &str) -> RouteParams {
    let mut params = RouteParams::new();
    params.insert(key.to_string(), value.to_string());
    params
}

/// Create RouteParams with multiple key-value pairs
pub fn params_with_multiple(pairs: Vec<(&str, &str)>) -> RouteParams {
    let mut params = RouteParams::new();
    for (key, value) in pairs {
        params.insert(key.to_string(), value.to_string());
    }
    params
}
