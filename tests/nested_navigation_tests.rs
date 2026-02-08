//! Integration tests for nested route navigation
//!
//! Tests T014-T019: Segment matching and nested navigation scenarios

use gpui::{IntoElement, ParentElement};
use gpui_navigator::*;
use std::sync::Arc;

// Helper function for test route builder
fn test_builder(
    _window: &mut gpui::Window,
    _app: &mut gpui::App,
    _params: &RouteParams,
) -> gpui::AnyElement {
    gpui::div().child("Test").into_any_element()
}

// T014: Static segment matching
#[test]
fn test_static_segment_exact_match() {
    let parent = Arc::new(
        Route::new("/dashboard", test_builder)
            .children(vec![Arc::new(Route::new("settings", test_builder))]),
    );
    let params = RouteParams::new();

    let result = resolve_child_route(&parent, "/dashboard/settings", &params, None);
    assert!(
        result.is_some(),
        "Should resolve static child segment 'settings'"
    );

    let (child, _) = result.unwrap();
    assert_eq!(child.config.path, "settings");
}

#[test]
fn test_static_segment_no_match() {
    let parent = Arc::new(
        Route::new("/dashboard", test_builder)
            .children(vec![Arc::new(Route::new("settings", test_builder))]),
    );
    let params = RouteParams::new();

    let result = resolve_child_route(&parent, "/dashboard/profile", &params, None);
    assert!(
        result.is_none(),
        "Should NOT resolve non-existent child segment"
    );
}

#[test]
fn test_static_segment_multiple_children() {
    let parent = Arc::new(Route::new("/dashboard", test_builder).children(vec![
        Arc::new(Route::new("overview", test_builder)),
        Arc::new(Route::new("settings", test_builder)),
        Arc::new(Route::new("profile", test_builder)),
    ]));
    let params = RouteParams::new();

    // Test each child
    let test_cases = vec![
        ("overview", "/dashboard/overview"),
        ("settings", "/dashboard/settings"),
        ("profile", "/dashboard/profile"),
    ];

    for (expected_path, full_path) in test_cases {
        let result = resolve_child_route(&parent, full_path, &params, None);
        assert!(result.is_some(), "Should resolve {}", expected_path);
        assert_eq!(result.unwrap().0.config.path, expected_path);
    }
}

// T015: Parameter segment matching
#[test]
fn test_parameter_segment_match() {
    let parent = Arc::new(
        Route::new("/users", test_builder)
            .children(vec![Arc::new(Route::new(":id", test_builder))]),
    );
    let params = RouteParams::new();

    let result = resolve_child_route(&parent, "/users/123", &params, None);
    assert!(result.is_some(), "Should resolve parameter segment");

    let (child, extracted_params) = result.unwrap();
    assert_eq!(child.config.path, ":id");
    assert_eq!(extracted_params.get("id"), Some(&"123".to_string()));
}

#[test]
fn test_parameter_segment_various_values() {
    let parent = Arc::new(
        Route::new("/posts", test_builder)
            .children(vec![Arc::new(Route::new(":slug", test_builder))]),
    );

    let test_values = vec!["hello-world", "123", "my-post-title", "a"];

    for slug in test_values {
        let params = RouteParams::new();
        let path = format!("/posts/{}", slug);
        let result = resolve_child_route(&parent, &path, &params, None);

        assert!(result.is_some(), "Should resolve for '{}'", slug);
        let (_, extracted) = result.unwrap();
        assert_eq!(extracted.get("slug"), Some(&slug.to_string()));
    }
}

#[test]
fn test_static_precedence_over_parameter() {
    let parent = Arc::new(Route::new("/items", test_builder).children(vec![
        Arc::new(Route::new("new", test_builder)),
        Arc::new(Route::new(":id", test_builder)),
    ]));
    let params = RouteParams::new();

    let result = resolve_child_route(&parent, "/items/new", &params, None);
    assert!(result.is_some());
    let (child, extracted) = result.unwrap();
    assert_eq!(child.config.path, "new");
    assert!(extracted.get("id").is_none());
}

// T016: Exact parent path prefix check
#[test]
fn test_parent_path_prefix_match() {
    let parent = Arc::new(
        Route::new("/dashboard", test_builder)
            .children(vec![Arc::new(Route::new("settings", test_builder))]),
    );
    let params = RouteParams::new();

    // Should match
    let result = resolve_child_route(&parent, "/dashboard/settings", &params, None);
    assert!(result.is_some(), "Should match with exact parent prefix");

    // Should NOT match different parent
    let result2 = resolve_child_route(&parent, "/profile/settings", &params, None);
    assert!(
        result2.is_none(),
        "Should NOT match different parent prefix"
    );
}

#[test]
fn test_partial_prefix_no_false_positive() {
    let parent = Arc::new(
        Route::new("/dash", test_builder)
            .children(vec![Arc::new(Route::new("settings", test_builder))]),
    );
    let params = RouteParams::new();

    // "/dashboard" should NOT match parent "/dash"
    let result = resolve_child_route(&parent, "/dashboard/settings", &params, None);
    assert!(result.is_none(), "Should NOT match partial prefix");
}

#[test]
fn test_root_parent_matches_all() {
    let parent = Arc::new(
        Route::new("/", test_builder).children(vec![Arc::new(Route::new("about", test_builder))]),
    );
    let params = RouteParams::new();

    let result = resolve_child_route(&parent, "/about", &params, None);
    assert!(result.is_some(), "Root parent should match child paths");
}

#[test]
fn test_trailing_slash_normalization() {
    let parent = Arc::new(
        Route::new("/dashboard/", test_builder)
            .children(vec![Arc::new(Route::new("settings", test_builder))]),
    );
    let params = RouteParams::new();

    let result = resolve_child_route(&parent, "/dashboard/settings", &params, None);
    assert!(result.is_some(), "Should handle trailing slash variations");
}

// T017: Shallow nesting (2 levels)
#[test]
fn test_shallow_nesting_two_levels() {
    // /parent -> /parent/child
    let parent = Arc::new(
        Route::new("/parent", test_builder)
            .children(vec![Arc::new(Route::new("child", test_builder))]),
    );
    let params = RouteParams::new();

    let result = resolve_child_route(&parent, "/parent/child", &params, None);
    assert!(result.is_some(), "Should resolve 2-level nesting");
    assert_eq!(result.unwrap().0.config.path, "child");
}

// T018: Deep nesting (3+ levels) - Note: current implementation only handles first child level
#[test]
fn test_deep_nesting_setup() {
    // Setup: /level1 -> /level1/level2 -> /level1/level2/level3
    let level3 = Arc::new(Route::new("level3", test_builder));
    let level2 = Arc::new(Route::new("level2", test_builder).children(vec![level3]));
    let level1 = Arc::new(Route::new("/level1", test_builder).children(vec![level2.clone()]));

    let params = RouteParams::new();

    // First level should work
    let result1 = resolve_child_route(&level1, "/level1/level2", &params, None);
    assert!(result1.is_some(), "Should resolve first child level");

    // For deeper levels, we need recursive resolution (will be implemented in T037)
    // This test documents current limitation
}

// T019: Index route navigation
#[test]
fn test_index_route_empty_path() {
    let parent = Arc::new(Route::new("/dashboard", test_builder).children(vec![
        Arc::new(Route::new("", test_builder)), // index route
    ]));
    let params = RouteParams::new();

    // Access parent path should find index route
    let result = resolve_child_route(&parent, "/dashboard", &params, None);
    assert!(result.is_some(), "Should find index route with empty path");
}

#[test]
fn test_index_route_explicit_index() {
    let parent = Arc::new(
        Route::new("/dashboard", test_builder)
            .children(vec![Arc::new(Route::new("index", test_builder))]),
    );
    let params = RouteParams::new();

    let result = resolve_child_route(&parent, "/dashboard", &params, None);
    assert!(
        result.is_some(),
        "Should find index route with 'index' path"
    );
}

#[test]
fn test_index_route_with_siblings() {
    let parent = Arc::new(Route::new("/dashboard", test_builder).children(vec![
        Arc::new(Route::new("", test_builder)), // index
        Arc::new(Route::new("settings", test_builder)),
    ]));
    let params = RouteParams::new();

    // Index route for parent path
    let result1 = resolve_child_route(&parent, "/dashboard", &params, None);
    assert!(result1.is_some(), "Should find index route");

    // Named sibling still works
    let result2 = resolve_child_route(&parent, "/dashboard/settings", &params, None);
    assert!(result2.is_some(), "Should still resolve named siblings");
    assert_eq!(result2.unwrap().0.config.path, "settings");
}
// T031-T034: Parameter inheritance tests

// T031: Recursive parameter extraction (multi-level nesting with params)

// T031: Recursive parameter extraction (multi-level nesting with params)
#[test]
fn test_recursive_parameter_extraction() {
    // /workspace/:wid/projects/:pid
    let root = Arc::new(
        Route::new("/workspace", test_builder).children(vec![Arc::new(
            Route::new(":wid", test_builder).children(vec![Arc::new(
                Route::new("projects", test_builder)
                    .children(vec![Arc::new(Route::new(":pid", test_builder))]),
            )]),
        )]),
    );

    // Single call should recursively resolve all levels
    let params = RouteParams::new();
    let result = resolve_child_route(&root, "/workspace/abc/projects/123", &params, None);
    assert!(
        result.is_some(),
        "Should resolve deeply nested route with multiple parameters"
    );

    let (_, final_params) = result.unwrap();
    assert_eq!(final_params.get("wid"), Some(&"abc".to_string()));
    assert_eq!(final_params.get("pid"), Some(&"123".to_string()));
}

// T032: Parameter inheritance (parent + child parameters merge)
#[test]
fn test_parameter_inheritance_merge() {
    let parent = Arc::new(Route::new("/parent", test_builder).children(vec![Arc::new(
        Route::new(":parent_id", test_builder)
            .children(vec![Arc::new(Route::new(":child_id", test_builder))]),
    )]));

    let params = RouteParams::new();
    let result = resolve_child_route(&parent, "/parent/p123/c456", &params, None);
    assert!(result.is_some(), "Should resolve two-level nested params");

    let (_, merged_params) = result.unwrap();
    assert_eq!(merged_params.get("parent_id"), Some(&"p123".to_string()));
    assert_eq!(merged_params.get("child_id"), Some(&"c456".to_string()));
}

// T033: Parameter name conflict (child overrides parent with same name)
#[test]
fn test_parameter_name_conflict_override() {
    // Both parent and child segments have :id parameter
    let parent = Arc::new(Route::new("/items", test_builder).children(vec![Arc::new(
        Route::new(":id", test_builder).children(vec![Arc::new(
                Route::new("details", test_builder)
                    .children(vec![Arc::new(Route::new(":id", test_builder))]),
            )]),
    )]));

    let params = RouteParams::new();
    let result = resolve_child_route(
        &parent,
        "/items/parent-123/details/child-456",
        &params,
        None,
    );
    assert!(
        result.is_some(),
        "Should resolve route with conflicting param names"
    );

    let (_, final_params) = result.unwrap();
    // Child :id should override parent :id
    assert_eq!(final_params.get("id"), Some(&"child-456".to_string()));
}

// T034: Integration test for full parameter inheritance flow
#[test]
fn test_full_parameter_inheritance_flow() {
    // /org/:org_id/team/:team_id/project/:project_id
    let root = Arc::new(Route::new("/org", test_builder).children(vec![Arc::new(
        Route::new(":org_id", test_builder).children(vec![Arc::new(
            Route::new("team", test_builder).children(vec![Arc::new(
                Route::new(":team_id", test_builder).children(vec![Arc::new(
                    Route::new("project", test_builder)
                        .children(vec![Arc::new(Route::new(":project_id", test_builder))]),
                )]),
            )]),
        )]),
    )]));

    let params = RouteParams::new();
    let result = resolve_child_route(
        &root,
        "/org/my-org/team/dev-team/project/proj-123",
        &params,
        None,
    );
    assert!(result.is_some(), "Should resolve 5-level deep nested route");

    let (_, final_params) = result.unwrap();
    assert_eq!(final_params.get("org_id"), Some(&"my-org".to_string()));
    assert_eq!(final_params.get("team_id"), Some(&"dev-team".to_string()));
    assert_eq!(
        final_params.get("project_id"),
        Some(&"proj-123".to_string())
    );
}

// T059: Integration test - Named outlets render independently
#[test]
fn test_named_outlets_independent() {
    // Create a parent with both default children and named outlet children
    let parent = Arc::new(
        Route::new("/app", test_builder)
            .children(vec![
                Arc::new(Route::new("dashboard", test_builder)),
                Arc::new(Route::new("settings", test_builder)),
            ])
            .named_outlet(
                "sidebar",
                vec![
                    Arc::new(Route::new("menu", test_builder)),
                    Arc::new(Route::new("notifications", test_builder)),
                ],
            ),
    );

    let params = RouteParams::new();

    // Test 1: Default outlet resolves to default children
    let default_dashboard = resolve_child_route(&parent, "/app/dashboard", &params, None);
    assert!(
        default_dashboard.is_some(),
        "Default outlet should resolve 'dashboard'"
    );
    let (route1, _) = default_dashboard.unwrap();
    assert_eq!(route1.config.path, "dashboard");

    let default_settings = resolve_child_route(&parent, "/app/settings", &params, None);
    assert!(
        default_settings.is_some(),
        "Default outlet should resolve 'settings'"
    );
    let (route2, _) = default_settings.unwrap();
    assert_eq!(route2.config.path, "settings");

    // Test 2: Named outlet "sidebar" resolves to its own children
    let sidebar_menu = resolve_child_route(&parent, "/app/menu", &params, Some("sidebar"));
    assert!(
        sidebar_menu.is_some(),
        "Named outlet 'sidebar' should resolve 'menu'"
    );
    let (route3, _) = sidebar_menu.unwrap();
    assert_eq!(route3.config.path, "menu");

    let sidebar_notif =
        resolve_child_route(&parent, "/app/notifications", &params, Some("sidebar"));
    assert!(
        sidebar_notif.is_some(),
        "Named outlet 'sidebar' should resolve 'notifications'"
    );
    let (route4, _) = sidebar_notif.unwrap();
    assert_eq!(route4.config.path, "notifications");

    // Test 3: Named outlet doesn't resolve default children
    let sidebar_dashboard =
        resolve_child_route(&parent, "/app/dashboard", &params, Some("sidebar"));
    assert!(
        sidebar_dashboard.is_none(),
        "Named outlet 'sidebar' should NOT resolve default child 'dashboard'"
    );

    // Test 4: Default outlet doesn't resolve named children
    let default_menu = resolve_child_route(&parent, "/app/menu", &params, None);
    assert!(
        default_menu.is_none(),
        "Default outlet should NOT resolve named child 'menu'"
    );
}
