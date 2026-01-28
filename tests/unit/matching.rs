//! Unit tests for segment-based path matching
//!
//! Tests for src/matching.rs - verifying exact match, param extraction,
//! and no-match scenarios as specified in T011.

#[cfg(test)]
mod matching_tests {
    use gpui::IntoElement;
    use gpui_navigator::matching::{match_path, split_path};
    use gpui_navigator::route::Route;
    use std::sync::Arc;

    #[test]
    fn test_exact_literal_match() {
        // T011 - Test exact literal segment matching
        // Example: "/users/list" matches route "/users/list"
        let route = Arc::new(Route::new("/users/list", |_, _, _| {
            gpui::div().into_any_element()
        }));

        let result = match_path("/users/list", &route);
        assert!(result.is_some());

        let matched = result.unwrap();
        assert_eq!(matched.route.config.path, "/users/list");
        assert!(matched.params.is_empty());
        assert!(matched.remaining.is_empty());
    }

    #[test]
    fn test_param_extraction() {
        // T011 - Test parameter extraction from :param segments
        // Example: "/users/123" matches route "/users/:id" with {id: "123"}
        let route = Arc::new(Route::new("/users/:id", |_, _, _| {
            gpui::div().into_any_element()
        }));

        let result = match_path("/users/123", &route);
        assert!(result.is_some());

        let matched = result.unwrap();
        assert_eq!(matched.params.get("id"), Some(&"123".to_string()));
        assert!(matched.remaining.is_empty());
    }

    #[test]
    fn test_multiple_params() {
        // T011 - Test multiple parameter extraction
        let route = Arc::new(Route::new(
            "/workspace/:workspaceId/project/:projectId",
            |_, _, _| gpui::div().into_any_element(),
        ));

        let result = match_path("/workspace/123/project/456", &route);
        assert!(result.is_some());

        let matched = result.unwrap();
        assert_eq!(matched.params.get("workspaceId"), Some(&"123".to_string()));
        assert_eq!(matched.params.get("projectId"), Some(&"456".to_string()));
        assert!(matched.remaining.is_empty());
    }

    #[test]
    fn test_no_match_wrong_literal() {
        // T011 - Test paths that don't match routes
        let route = Arc::new(Route::new("/users/list", |_, _, _| {
            gpui::div().into_any_element()
        }));

        let result = match_path("/users/create", &route);
        assert!(result.is_none());
    }

    #[test]
    fn test_no_match_too_short() {
        // T011 - Path shorter than route pattern
        let route = Arc::new(Route::new("/users/:id/profile", |_, _, _| {
            gpui::div().into_any_element()
        }));

        let result = match_path("/users/123", &route);
        assert!(result.is_none());
    }

    #[test]
    fn test_remaining_segments_for_nested() {
        // T011 - Test remaining unmatched segments for nested resolution
        // Example: "/users/123/profile" matches "/users/:id" with remaining ["profile"]
        let route = Arc::new(Route::new("/users/:id", |_, _, _| {
            gpui::div().into_any_element()
        }));

        let result = match_path("/users/123/profile", &route);
        assert!(result.is_some());

        let matched = result.unwrap();
        assert_eq!(matched.params.get("id"), Some(&"123".to_string()));
        assert_eq!(matched.remaining, vec!["profile"]);
    }

    #[test]
    fn test_remaining_multiple_segments() {
        // T011 - Multiple remaining segments
        let route = Arc::new(Route::new("/dashboard", |_, _, _| {
            gpui::div().into_any_element()
        }));

        let result = match_path("/dashboard/analytics/overview", &route);
        assert!(result.is_some());

        let matched = result.unwrap();
        assert_eq!(matched.remaining, vec!["analytics", "overview"]);
    }

    #[test]
    fn test_split_path_variants() {
        // T011 - Test path splitting edge cases
        assert_eq!(split_path("/users/123"), vec!["users", "123"]);
        assert_eq!(split_path("/"), Vec::<String>::new());
        assert_eq!(split_path(""), Vec::<String>::new());
        assert_eq!(split_path("/users/"), vec!["users"]);
        assert_eq!(split_path("users/123/"), vec!["users", "123"]);
    }

    #[test]
    fn test_index_route_empty_path() {
        // T011 - Index routes have empty path ""
        let route = Arc::new(Route::new("", |_, _, _| gpui::div().into_any_element()));

        let result = match_path("/", &route);
        assert!(result.is_some());

        let matched = result.unwrap();
        assert!(matched.params.is_empty());
        assert!(matched.remaining.is_empty());
    }
}
