//! Unit tests for named outlet resolution
//!
//! Tests the named outlet functionality for multi-panel layouts (e.g., main + sidebar)

use gpui_navigator::nested::resolve_child_route;
use gpui_navigator::{Route, RouteParams};
use std::sync::Arc;

fn test_builder(
    _window: &mut gpui::Window,
    _app: &mut gpui::App,
    _params: &RouteParams,
) -> gpui::AnyElement {
    gpui::div().into_any_element()
}

use gpui::IntoElement;

// T056: Named outlet resolution
#[test]
fn test_named_outlet_resolution() {
    // Create parent with named children for "sidebar" outlet
    let parent = Arc::new(Route::new("/dashboard", test_builder).named_outlet(
        "sidebar",
        vec![Arc::new(Route::new("settings", test_builder))],
    ));

    let params = RouteParams::new();

    // Resolve with named outlet
    let result = resolve_child_route(&parent, "/dashboard/settings", &params, Some("sidebar"));
    assert!(
        result.is_some(),
        "Should resolve named outlet 'sidebar' child"
    );

    let (route, _) = result.unwrap();
    assert_eq!(route.config.path, "settings");
}

// T057: Missing named outlet returns None
#[test]
fn test_missing_named_outlet() {
    // Create parent with only default children
    let parent = Arc::new(
        Route::new("/dashboard", test_builder)
            .children(vec![Arc::new(Route::new("home", test_builder))]),
    );

    let params = RouteParams::new();

    // Try to resolve non-existent named outlet
    let result = resolve_child_route(&parent, "/dashboard/settings", &params, Some("sidebar"));
    assert!(
        result.is_none(),
        "Should return None for missing named outlet"
    );
}

// T058: Default outlet vs named outlet
#[test]
fn test_default_vs_named_outlet() {
    // Create parent with both default and named children
    let parent = Arc::new(
        Route::new("/dashboard", test_builder)
            .children(vec![Arc::new(Route::new("home", test_builder))])
            .named_outlet(
                "sidebar",
                vec![Arc::new(Route::new("settings", test_builder))],
            ),
    );

    let params = RouteParams::new();

    // Resolve default outlet (None)
    let default_result = resolve_child_route(&parent, "/dashboard/home", &params, None);
    assert!(default_result.is_some(), "Should resolve default outlet");
    let (default_route, _) = default_result.unwrap();
    assert_eq!(default_route.config.path, "home");

    // Resolve named outlet
    let named_result =
        resolve_child_route(&parent, "/dashboard/settings", &params, Some("sidebar"));
    assert!(named_result.is_some(), "Should resolve named outlet");
    let (named_route, _) = named_result.unwrap();
    assert_eq!(named_route.config.path, "settings");

    // Verify they're different routes
    assert_ne!(
        default_route.config.path, named_route.config.path,
        "Default and named outlets should resolve to different routes"
    );
}
