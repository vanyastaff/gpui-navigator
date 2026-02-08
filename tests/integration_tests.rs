//! Integration tests for gpui_navigator
//!
//! These tests verify the complete router workflow including initialization,
//! navigation, guards, and route matching.

#![allow(clippy::future_not_send, clippy::unused_async)]

use gpui::{div, IntoElement, ParentElement, TestAppContext};
use gpui_navigator::*;

// ============================================================================
// Router Initialization Tests
// ============================================================================

#[gpui::test]
async fn test_router_initialization(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(Route::new("/", |_, _, _| {
                div().child("Home").into_any_element()
            }));
            router.add_route(Route::new("/about", |_, _, _| {
                div().child("About").into_any_element()
            }));
        });
    });

    let path = cx.read(Navigator::current_path);
    assert_eq!(path, "/");
}

#[gpui::test]
async fn test_router_with_named_routes(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(
                Route::new("/users/:id", |_, _, params| {
                    let id = params.get("id").cloned().unwrap_or_default();
                    div().child(format!("User {}", id)).into_any_element()
                })
                .name("user-profile"),
            );
        });
    });

    // Test URL generation
    let mut params = RouteParams::new();
    params.set("id".to_string(), "123".to_string());

    let url = cx.read(|cx| Navigator::url_for(cx, "user-profile", &params));
    assert_eq!(url, Some("/users/123".to_string()));
}

// ============================================================================
// Navigation Tests
// ============================================================================

#[gpui::test]
async fn test_push_navigation(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(Route::new("/", |_, _, _| div().into_any_element()));
            router.add_route(Route::new("/page1", |_, _, _| div().into_any_element()));
            router.add_route(Route::new("/page2", |_, _, _| div().into_any_element()));
        });
    });

    // Navigate to page1
    cx.update(|cx| Navigator::push(cx, "/page1"));
    assert_eq!(cx.read(Navigator::current_path), "/page1");

    // Navigate to page2
    cx.update(|cx| Navigator::push(cx, "/page2"));
    assert_eq!(cx.read(Navigator::current_path), "/page2");

    // Can go back
    assert!(cx.read(Navigator::can_pop));
}

#[gpui::test]
async fn test_pop_navigation(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(Route::new("/", |_, _, _| div().into_any_element()));
            router.add_route(Route::new("/page1", |_, _, _| div().into_any_element()));
        });
    });

    // Push and pop
    cx.update(|cx| Navigator::push(cx, "/page1"));
    assert_eq!(cx.read(Navigator::current_path), "/page1");

    cx.update(Navigator::pop);
    assert_eq!(cx.read(Navigator::current_path), "/");

    // Can't pop past initial route
    assert!(!cx.read(Navigator::can_pop));
}

#[gpui::test]
async fn test_replace_navigation(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(Route::new("/", |_, _, _| div().into_any_element()));
            router.add_route(Route::new("/login", |_, _, _| div().into_any_element()));
            router.add_route(Route::new("/dashboard", |_, _, _| div().into_any_element()));
        });
    });

    // Navigate then replace
    cx.update(|cx| Navigator::push(cx, "/login"));
    cx.update(|cx| Navigator::replace(cx, "/dashboard"));

    assert_eq!(cx.read(Navigator::current_path), "/dashboard");

    // Pop should go back to home, not login
    cx.update(Navigator::pop);
    assert_eq!(cx.read(Navigator::current_path), "/");
}

#[gpui::test]
async fn test_forward_navigation(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(Route::new("/", |_, _, _| div().into_any_element()));
            router.add_route(Route::new("/page1", |_, _, _| div().into_any_element()));
        });
    });

    cx.update(|cx| Navigator::push(cx, "/page1"));
    cx.update(Navigator::pop);

    // Should be able to go forward
    assert!(cx.read(Navigator::can_go_forward));

    cx.update(Navigator::forward);
    assert_eq!(cx.read(Navigator::current_path), "/page1");
}

// ============================================================================
// Route Parameters Tests
// ============================================================================

#[gpui::test]
async fn test_route_params_extraction(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(Route::new("/users/:id", |_, _, params| {
                let id = params.get("id").cloned().unwrap_or_default();
                div().child(format!("User: {}", id)).into_any_element()
            }));
        });
    });

    cx.update(|cx| Navigator::push(cx, "/users/42"));

    // Verify route matched
    assert_eq!(cx.read(Navigator::current_path), "/users/42");
}

#[test]
fn test_route_params_type_conversion() {
    let mut params = RouteParams::new();
    params.insert("id".to_string(), "123".to_string());
    params.insert("active".to_string(), "true".to_string());
    params.insert("invalid".to_string(), "not_a_number".to_string());

    assert_eq!(params.get_as::<i32>("id"), Some(123));
    assert_eq!(params.get_as::<bool>("active"), Some(true));
    assert_eq!(params.get_as::<i32>("invalid"), None);
    assert_eq!(params.get_as::<i32>("missing"), None);
}

// ============================================================================
// Query Parameters Tests
// ============================================================================

#[test]
fn test_query_params_parsing() {
    let query = QueryParams::from_query_string("page=1&sort=name&active=true");

    assert_eq!(query.get("page"), Some(&"1".to_string()));
    assert_eq!(query.get("sort"), Some(&"name".to_string()));
    assert_eq!(query.get_as::<i32>("page"), Some(1));
    assert_eq!(query.get_as::<bool>("active"), Some(true));
}

#[test]
fn test_query_params_multiple_values() {
    let query = QueryParams::from_query_string("tag=rust&tag=gpui&tag=router");

    let tags = query.get_all("tag").unwrap();
    assert_eq!(tags.len(), 3);
    assert!(tags.contains(&"rust".to_string()));
    assert!(tags.contains(&"gpui".to_string()));
    assert!(tags.contains(&"router".to_string()));
}

// ============================================================================
// Guard Tests
// ============================================================================

#[gpui::test]
async fn test_auth_guard_allows_authenticated(cx: &mut TestAppContext) {
    let guard = AuthGuard::new(|_| true, "/login");
    let request = NavigationRequest::new("/protected".to_string());

    let result = cx.update(|cx| guard.check(cx, &request));

    assert!(result.is_continue());
}

#[gpui::test]
async fn test_auth_guard_redirects_unauthenticated(cx: &mut TestAppContext) {
    let guard = AuthGuard::new(|_| false, "/login");
    let request = NavigationRequest::new("/protected".to_string());

    let result = cx.update(|cx| guard.check(cx, &request));

    assert!(result.is_redirect());
    assert_eq!(result.redirect_path(), Some("/login"));
}

#[gpui::test]
async fn test_role_guard_allows_correct_role(cx: &mut TestAppContext) {
    let guard = RoleGuard::new(|_| Some("admin".to_string()), "admin", None::<String>);
    let request = NavigationRequest::new("/admin".to_string());

    let result = cx.update(|cx| guard.check(cx, &request));

    assert!(result.is_continue());
}

#[gpui::test]
async fn test_role_guard_denies_wrong_role(cx: &mut TestAppContext) {
    let guard = RoleGuard::new(|_| Some("user".to_string()), "admin", None::<String>);
    let request = NavigationRequest::new("/admin".to_string());

    let result = cx.update(|cx| guard.check(cx, &request));

    assert!(result.is_deny());
}

#[gpui::test]
async fn test_permission_guard(cx: &mut TestAppContext) {
    let guard = PermissionGuard::new(|_, perm| perm == "users.read", "users.read");
    let request = NavigationRequest::new("/users".to_string());

    let result = cx.update(|cx| guard.check(cx, &request));

    assert!(result.is_continue());
}

// ============================================================================
// NavigationAction Tests
// ============================================================================

#[test]
fn test_navigation_action_variants() {
    let cont = NavigationAction::allow();
    assert!(cont.is_continue());
    assert!(!cont.is_deny());
    assert!(!cont.is_redirect());

    let deny = NavigationAction::deny("Forbidden");
    assert!(!deny.is_continue());
    assert!(deny.is_deny());

    let redirect = NavigationAction::redirect("/login");
    assert!(redirect.is_redirect());
    assert_eq!(redirect.redirect_path(), Some("/login"));
}

// ============================================================================
// Transition Tests
// ============================================================================

#[test]
fn test_transition_duration() {
    use std::time::Duration;

    assert_eq!(Transition::None.duration(), Duration::ZERO);
    assert_eq!(Transition::fade(300).duration(), Duration::from_millis(300));
    assert_eq!(
        Transition::slide_left(400).duration(),
        Duration::from_millis(400)
    );
    assert_eq!(
        Transition::slide_up(500).duration(),
        Duration::from_millis(500)
    );
}

#[test]
fn test_transition_is_none() {
    assert!(Transition::None.is_none());
    assert!(!Transition::fade(200).is_none());
}

// ============================================================================
// Named Route Registry Tests
// ============================================================================

#[test]
fn test_named_route_registry() {
    let mut registry = NamedRouteRegistry::new();
    registry.register("home", "/");
    registry.register("user", "/users/:id");

    assert!(registry.contains("home"));
    assert!(registry.contains("user"));
    assert!(!registry.contains("unknown"));

    assert_eq!(registry.get("home"), Some("/"));
    assert_eq!(registry.get("user"), Some("/users/:id"));
}

#[test]
fn test_named_route_url_generation() {
    let mut registry = NamedRouteRegistry::new();
    registry.register("user", "/users/:id");
    registry.register("post", "/users/:userId/posts/:postId");

    let mut params = RouteParams::new();
    params.set("id".to_string(), "42".to_string());

    let url = registry.url_for("user", &params);
    assert_eq!(url, Some("/users/42".to_string()));

    let mut params = RouteParams::new();
    params.set("userId".to_string(), "1".to_string());
    params.set("postId".to_string(), "99".to_string());

    let url = registry.url_for("post", &params);
    assert_eq!(url, Some("/users/1/posts/99".to_string()));
}

// ============================================================================
// Navigation Result Tests
// ============================================================================

#[test]
fn test_navigation_result_variants() {
    let success = NavigationResult::Success {
        path: "/home".to_string(),
    };
    assert!(success.is_success());
    assert!(!success.is_not_found());

    let not_found = NavigationResult::NotFound {
        path: "/unknown".to_string(),
    };
    assert!(not_found.is_not_found());

    let blocked = NavigationResult::Blocked {
        reason: "Not authorized".to_string(),
        redirect: Some("/login".to_string()),
    };
    assert!(blocked.is_blocked());
    assert_eq!(blocked.redirect_path(), Some("/login"));
}

// ============================================================================
// History Tests
// ============================================================================

#[test]
fn test_history_operations() {
    use gpui_navigator::history::History;

    let mut history = History::new("/".to_string());

    assert_eq!(history.current_path(), "/");
    assert!(!history.can_go_back());
    assert!(!history.can_go_forward());

    history.push("/page1".to_string());
    assert_eq!(history.current_path(), "/page1");
    assert!(history.can_go_back());

    history.push("/page2".to_string());
    history.back();
    assert_eq!(history.current_path(), "/page1");
    assert!(history.can_go_forward());

    history.forward();
    assert_eq!(history.current_path(), "/page2");
}

#[test]
fn test_history_truncation() {
    use gpui_navigator::history::History;

    let mut history = History::new("/".to_string());
    history.push("/page1".to_string());
    history.push("/page2".to_string());
    history.back(); // At /page1

    // Push new route - should truncate /page2
    history.push("/page3".to_string());

    assert!(!history.can_go_forward());
    history.back();
    assert_eq!(history.current_path(), "/page1");
}

// ============================================================================
// Route Matching Tests
// ============================================================================

#[gpui::test]
async fn test_static_route_matching(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(Route::new("/about", |_, _, _| {
                div().child("About").into_any_element()
            }));
        });
    });

    cx.update(|cx| Navigator::push(cx, "/about"));
    assert_eq!(cx.read(Navigator::current_path), "/about");
}

#[gpui::test]
async fn test_dynamic_route_matching(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(Route::new("/posts/:id", |_, _, _| {
                div().child("Post").into_any_element()
            }));
        });
    });

    cx.update(|cx| Navigator::push(cx, "/posts/123"));
    assert_eq!(cx.read(Navigator::current_path), "/posts/123");
}

// ============================================================================
// Lifecycle Tests
// ============================================================================

#[test]
fn test_lifecycle_navigation_action_variants() {
    let cont = NavigationAction::allow();
    assert!(cont.is_continue());
    assert!(!cont.is_deny());
    assert!(!cont.is_redirect());

    let deny = NavigationAction::deny("Unsaved changes");
    assert!(!deny.is_continue());
    assert!(deny.is_deny());

    let redirect = NavigationAction::redirect("/login");
    assert!(!redirect.is_continue());
    assert!(redirect.is_redirect());
}

// ============================================================================
// Cache Tests
// ============================================================================

#[test]
fn test_route_cache_stats() {
    let mut cache = RouteCache::new();

    // Initial stats should be zero
    assert_eq!(cache.stats().parent_hits, 0);
    assert_eq!(cache.stats().parent_misses, 0);

    // Miss
    cache.get_parent("/test");
    assert_eq!(cache.stats().parent_misses, 1);

    // Set and hit
    cache.set_parent("/test".to_string(), gpui_navigator::RouteId::from_path("/"));
    cache.get_parent("/test");
    assert_eq!(cache.stats().parent_hits, 1);

    // Clear
    cache.clear();
    assert_eq!(cache.stats().invalidations, 1);
    assert_eq!(cache.parent_cache_size(), 0);
}

// ============================================================================
// Error Handlers Tests
// ============================================================================

#[gpui::test]
async fn test_not_found_handler_rendering(cx: &mut TestAppContext) {
    let handlers = ErrorHandlers::new().on_not_found(|_cx, path| {
        div()
            .child(format!("Custom 404: Page '{}' not found", path))
            .into_any_element()
    });

    // Test that the handler can render a not found page
    let element = cx.update(|cx| handlers.render_not_found(cx, "/non-existent-page"));
    assert!(element.is_some());
}

#[gpui::test]
async fn test_error_handler_rendering(cx: &mut TestAppContext) {
    use gpui_navigator::NavigationError;

    let handlers = ErrorHandlers::new()
        .on_not_found(|_cx, path| {
            div()
                .child(format!("404: {} not found", path))
                .into_any_element()
        })
        .on_error(|_cx, error| {
            div()
                .child(format!("Error occurred: {}", error))
                .into_any_element()
        });

    // Test not found handler
    let not_found_element = cx.update(|cx| handlers.render_not_found(cx, "/invalid"));
    assert!(not_found_element.is_some());

    // Test error handler
    let error = NavigationError::RouteNotFound {
        path: "/test".to_string(),
    };
    let error_element = cx.update(|cx| handlers.render_error(cx, &error));
    assert!(error_element.is_some());
}

#[gpui::test]
async fn test_custom_error_pages(cx: &mut TestAppContext) {
    use gpui_navigator::NavigationError;

    let handlers = ErrorHandlers::new()
        .on_not_found(|_cx, path| {
            div()
                .child("Error 404")
                .child(div().child(format!("The page '{}' could not be found", path)))
                .child(div().child("Go back to home"))
                .into_any_element()
        })
        .on_error(|_cx, error| {
            div()
                .child("Application Error")
                .child(div().child(format!("Details: {}", error)))
                .into_any_element()
        });

    // Test 404 page with complex layout
    let element = cx.update(|cx| handlers.render_not_found(cx, "/does-not-exist"));
    assert!(element.is_some());

    // Test error page with navigation error
    let error = NavigationError::GuardBlocked {
        reason: "Not authenticated".to_string(),
    };
    let element = cx.update(|cx| handlers.render_error(cx, &error));
    assert!(element.is_some());
}

// ============================================================================
// Integration: Full Navigation Flow
// ============================================================================

#[gpui::test]
async fn test_full_navigation_flow(cx: &mut TestAppContext) {
    // Initialize router with multiple routes
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(
                Route::new("/", |_, _, _| div().child("Home").into_any_element())
                    .name("home")
                    .transition(Transition::None),
            );
            router.add_route(
                Route::new("/users", |_, _, _| {
                    div().child("Users List").into_any_element()
                })
                .name("users")
                .transition(Transition::fade(200)),
            );
            router.add_route(
                Route::new("/users/:id", |_, _, params| {
                    let id = params.get("id").cloned().unwrap_or_default();
                    div().child(format!("User {}", id)).into_any_element()
                })
                .name("user-detail")
                .transition(Transition::slide_left(300)),
            );
        });
    });

    // Start at home
    assert_eq!(cx.read(Navigator::current_path), "/");
    assert!(!cx.read(Navigator::can_pop));

    // Navigate to users list
    cx.update(|cx| Navigator::push(cx, "/users"));
    assert_eq!(cx.read(Navigator::current_path), "/users");
    assert!(cx.read(Navigator::can_pop));

    // Navigate to specific user
    cx.update(|cx| Navigator::push(cx, "/users/42"));
    assert_eq!(cx.read(Navigator::current_path), "/users/42");

    // Go back twice
    cx.update(Navigator::pop);
    assert_eq!(cx.read(Navigator::current_path), "/users");

    cx.update(Navigator::pop);
    assert_eq!(cx.read(Navigator::current_path), "/");

    // Go forward
    cx.update(Navigator::forward);
    assert_eq!(cx.read(Navigator::current_path), "/users");

    // Navigate by name
    let mut params = RouteParams::new();
    params.set("id".to_string(), "99".to_string());
    cx.update(|cx| Navigator::push_named(cx, "user-detail", &params));
    assert_eq!(cx.read(Navigator::current_path), "/users/99");
}
