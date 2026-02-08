//! Unit tests for hierarchical route resolution
//!
//! Tests for src/nested.rs - verifying parent+child matching,
//! index route selection as specified in T019 and T042.

#[cfg(test)]
mod nested_tests {
    use gpui::{IntoElement, ParentElement};
    use gpui_navigator::nested::{normalize_path, resolve_child_route};
    use gpui_navigator::route::Route;
    use gpui_navigator::RouteParams;
    use std::sync::Arc;

    #[test]
    fn test_parent_child_matching() {
        // T019 - Test hierarchical resolution
        // Example: "/dashboard/analytics" resolves parent "/dashboard" then child "analytics"

        let overview_route = Route::new("overview", |_, _, _| gpui::div().into_any_element());
        let analytics_route = Route::new("analytics", |_, _, _| gpui::div().into_any_element());
        let settings_route = Route::new("settings", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element()).children(vec![
                overview_route.into(),
                analytics_route.into(),
                settings_route.into(),
            ]),
        );

        let parent_params = RouteParams::new();

        // Resolve "analytics" child
        let result =
            resolve_child_route(&parent_route, "/dashboard/analytics", &parent_params, None);
        assert!(result.is_some());

        let (child_route, child_params) = result.unwrap();
        assert_eq!(child_route.config.path, "analytics");
        assert!(child_params.is_empty());
    }

    #[test]
    fn test_index_route_selection() {
        // T019, T042 - Test finding child with empty path "" as default
        // Example: navigate "/dashboard" → renders index child with path ""

        let index_route = Route::new("", |_, _, _| gpui::div().into_any_element());
        let analytics_route = Route::new("analytics", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element())
                .children(vec![index_route.into(), analytics_route.into()]),
        );

        let parent_params = RouteParams::new();

        // Navigate to parent without specifying child - should get index
        let result = resolve_child_route(&parent_route, "/dashboard", &parent_params, None);
        assert!(result.is_some());

        let (child_route, _) = result.unwrap();
        assert_eq!(child_route.config.path, ""); // Index route has empty path
    }

    #[test]
    fn test_index_route_with_root_path() {
        // T042 - Navigate "/" → renders root index child
        let index_route = Route::new("", |_, _, _| gpui::div().into_any_element());
        let about_route = Route::new("about", |_, _, _| gpui::div().into_any_element());

        let root_route = Arc::new(
            Route::new("/", |_, _, _| gpui::div().into_any_element())
                .children(vec![index_route.into(), about_route.into()]),
        );

        let parent_params = RouteParams::new();

        let result = resolve_child_route(&root_route, "/", &parent_params, None);
        assert!(result.is_some());

        let (child_route, _) = result.unwrap();
        assert_eq!(child_route.config.path, "");
    }

    #[test]
    fn test_deep_hierarchy() {
        // T035 - Test 4+ level nesting without recursion
        // /app/workspace/:workspaceId/project/:projectId

        let task_route = Route::new(":taskId", |_, _, _| gpui::div().into_any_element());
        let project_route = Route::new(":projectId", |_, _, _| gpui::div().into_any_element())
            .children(vec![task_route.into()]);
        let workspace_route = Route::new(":workspaceId", |_, _, _| gpui::div().into_any_element())
            .children(vec![project_route.into()]);
        let app_route = Arc::new(
            Route::new("/app", |_, _, _| gpui::div().into_any_element())
                .children(vec![workspace_route.into()]),
        );

        let parent_params = RouteParams::new();

        // Resolve to taskId level
        let result = resolve_child_route(
            &app_route,
            "/app/ws-123/proj-456/task-789",
            &parent_params,
            None,
        );

        assert!(result.is_some());
        let (_child_route, child_params) = result.unwrap();

        // Should have extracted all parameters
        assert_eq!(child_params.get("workspaceId"), Some(&"ws-123".to_string()));
        assert_eq!(child_params.get("projectId"), Some(&"proj-456".to_string()));
        assert_eq!(child_params.get("taskId"), Some(&"task-789".to_string()));
    }

    #[test]
    fn test_no_matching_child() {
        // T019 - No child matches path
        let overview_route = Route::new("overview", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element())
                .children(vec![overview_route.into()]),
        );

        let parent_params = RouteParams::new();

        // Try to resolve non-existent "analytics" child
        let result =
            resolve_child_route(&parent_route, "/dashboard/analytics", &parent_params, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_normalize_path_variants() {
        // T019 - Test path normalization
        assert_eq!(normalize_path("/dashboard"), "/dashboard");
        assert_eq!(normalize_path("dashboard"), "/dashboard");
        assert_eq!(normalize_path("/dashboard/"), "/dashboard");
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path(""), "/");
    }

    #[test]
    fn test_child_with_params() {
        // T019 - Child route with parameter
        let detail_route = Route::new(":id", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/users", |_, _, _| gpui::div().into_any_element())
                .children(vec![detail_route.into()]),
        );

        let parent_params = RouteParams::new();

        let result = resolve_child_route(&parent_route, "/users/123", &parent_params, None);
        assert!(result.is_some());

        let (child_route, child_params) = result.unwrap();
        assert_eq!(child_route.config.path, ":id");
        assert_eq!(child_params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_param_inheritance() {
        // T019 - Child inherits parent params
        // This is tested through the deep_hierarchy test above
        // which demonstrates 3-level parameter inheritance

        // Simple 2-level test
        let detail_route = Route::new(":id", |_, _, _| gpui::div().into_any_element());
        let users_route = Arc::new(
            Route::new("/users", |_, _, _| gpui::div().into_any_element())
                .children(vec![detail_route.into()]),
        );

        let mut parent_params = RouteParams::new();
        parent_params.set("category".to_string(), "admin".to_string());

        let result = resolve_child_route(&users_route, "/users/123", &parent_params, None);

        assert!(result.is_some());
        let (_, child_params) = result.unwrap();

        // Child should have its own param
        assert_eq!(child_params.get("id"), Some(&"123".to_string()));
        // And should have inherited parent param
        assert_eq!(child_params.get("category"), Some(&"admin".to_string()));
    }

    // T042: Additional index route tests for Phase 6

    #[test]
    fn test_index_route_priority_over_named() {
        // T042 - Index route (empty path) should have priority over "index" named route
        let empty_index = Route::new("", |_, _, _| {
            gpui::div().child("Empty Index").into_any_element()
        });
        let named_index = Route::new("index", |_, _, _| {
            gpui::div().child("Named Index").into_any_element()
        });
        let other_route = Route::new("other", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element()).children(vec![
                other_route.into(),
                named_index.into(),
                empty_index.into(),
            ]),
        );

        let parent_params = RouteParams::new();
        let result = resolve_child_route(&parent_route, "/dashboard", &parent_params, None);

        assert!(result.is_some());
        let (child_route, _) = result.unwrap();
        // Should prioritize empty path over "index"
        assert_eq!(child_route.config.path, "");
    }

    #[test]
    fn test_index_route_with_siblings() {
        // T042 - Index route works correctly alongside named child routes
        let index_route = Route::new("", |_, _, _| gpui::div().into_any_element());
        let overview = Route::new("overview", |_, _, _| gpui::div().into_any_element());
        let settings = Route::new("settings", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element()).children(vec![
                index_route.into(),
                overview.into(),
                settings.into(),
            ]),
        );

        let parent_params = RouteParams::new();

        // Navigate to parent - should get index
        let result = resolve_child_route(&parent_route, "/dashboard", &parent_params, None);
        assert!(result.is_some());
        let (child_route, _) = result.unwrap();
        assert_eq!(child_route.config.path, "");

        // Navigate to explicit child - should get that child
        let result =
            resolve_child_route(&parent_route, "/dashboard/settings", &parent_params, None);
        assert!(result.is_some());
        let (child_route, _) = result.unwrap();
        assert_eq!(child_route.config.path, "settings");
    }

    #[test]
    fn test_no_index_route_returns_none() {
        // T042 - If parent has no index route, navigating to parent path returns None
        let overview = Route::new("overview", |_, _, _| gpui::div().into_any_element());
        let settings = Route::new("settings", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element())
                .children(vec![overview.into(), settings.into()]),
        );

        let parent_params = RouteParams::new();

        // Navigate to parent without index route - should return None
        let result = resolve_child_route(&parent_route, "/dashboard", &parent_params, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_nested_index_routes() {
        // T042 - Index routes work at multiple nesting levels
        let level3_index = Route::new("", |_, _, _| gpui::div().into_any_element());
        let level3_child = Route::new("child", |_, _, _| gpui::div().into_any_element());

        let level2_index = Route::new("", |_, _, _| gpui::div().into_any_element())
            .children(vec![level3_index.into(), level3_child.into()]);
        let level2_other = Route::new("other", |_, _, _| gpui::div().into_any_element());

        let level1 = Arc::new(
            Route::new("/parent", |_, _, _| gpui::div().into_any_element())
                .children(vec![level2_index.into(), level2_other.into()]),
        );

        let parent_params = RouteParams::new();

        // Navigate to parent - should resolve to level2 index
        let result = resolve_child_route(&level1, "/parent", &parent_params, None);
        assert!(result.is_some());
        let (child_route, _) = result.unwrap();
        assert_eq!(child_route.config.path, "");
    }

    #[test]
    fn test_index_route_with_trailing_slash() {
        // T042 - Index route resolution works with trailing slashes
        let index_route = Route::new("", |_, _, _| gpui::div().into_any_element());
        let other = Route::new("other", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element())
                .children(vec![index_route.into(), other.into()]),
        );

        let parent_params = RouteParams::new();

        // Both "/dashboard" and "/dashboard/" should resolve to index
        let result = resolve_child_route(&parent_route, "/dashboard", &parent_params, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0.config.path, "");

        let result = resolve_child_route(&parent_route, "/dashboard/", &parent_params, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0.config.path, "");
    }
}
