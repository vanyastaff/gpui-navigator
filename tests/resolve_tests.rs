//! Unit tests for resolve module (`MatchStack` resolution)
//!
//! Standalone test crate to avoid compiler stack overflow from deep generic
//! expansion of `Route::new()` when compiled with all other tests.

use gpui::{div, AnyElement, App, IntoElement, ParentElement, Window};
use gpui_navigator::resolve::*;
use gpui_navigator::route::Route;
use gpui_navigator::RouteParams;
use std::sync::Arc;

fn dummy(_window: &mut Window, _cx: &mut App, _params: &RouteParams) -> AnyElement {
    div().child("test").into_any_element()
}

// ---- resolve_match_stack tests ----

#[test]
fn test_flat_routes() {
    let routes = vec![
        Arc::new(Route::new("/", dummy)),
        Arc::new(Route::new("/about", dummy)),
        Arc::new(Route::new("/contact", dummy)),
    ];

    let stack = resolve_match_stack(&routes, "/about");
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/about");
}

#[test]
fn test_root_path() {
    let routes = vec![Arc::new(Route::new("/", dummy))];

    let stack = resolve_match_stack(&routes, "/");
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/");
}

#[test]
fn test_nested_two_levels() {
    let routes = vec![Arc::new(Route::new("/dashboard", dummy).children(vec![
        Arc::new(Route::new("", dummy)),
        Arc::new(Route::new("settings", dummy)),
    ]))];

    // Navigate to /dashboard → should match dashboard + index
    let stack = resolve_match_stack(&routes, "/dashboard");
    assert_eq!(stack.len(), 2);
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/dashboard");
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "");

    // Navigate to /dashboard/settings
    let stack = resolve_match_stack(&routes, "/dashboard/settings");
    assert_eq!(stack.len(), 2);
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/dashboard");
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "settings");
}

#[test]
fn test_nested_three_levels() {
    let routes = vec![Arc::new(Route::new("/dashboard", dummy).children(vec![
        Arc::new(Route::new("settings", dummy).children(vec![
            Arc::new(Route::new("profile", dummy)),
            Arc::new(Route::new("security", dummy)),
        ])),
    ]))];

    let stack = resolve_match_stack(&routes, "/dashboard/settings/profile");
    assert_eq!(stack.len(), 3);
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/dashboard");
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "settings");
    assert_eq!(stack.at_depth(2).unwrap().route.config.path, "profile");
}

#[test]
fn test_root_layout_with_children() {
    let routes = vec![Arc::new(Route::new("/", dummy).children(vec![
        Arc::new(Route::new("", dummy)),
        Arc::new(
            Route::new("dashboard", dummy).children(vec![Arc::new(Route::new("settings", dummy))]),
        ),
    ]))];

    // Navigate to /
    let stack = resolve_match_stack(&routes, "/");
    assert_eq!(stack.len(), 2); // root + index
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/");
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "");

    // Navigate to /dashboard/settings
    let stack = resolve_match_stack(&routes, "/dashboard/settings");
    assert_eq!(stack.len(), 3); // root + dashboard + settings
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/");
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "dashboard");
    assert_eq!(stack.at_depth(2).unwrap().route.config.path, "settings");
}

#[test]
fn test_parameter_routes() {
    let routes = vec![Arc::new(Route::new("/users/:id", dummy))];

    let stack = resolve_match_stack(&routes, "/users/42");
    assert_eq!(stack.len(), 1);
    assert_eq!(
        stack.at_depth(0).unwrap().params.get("id"),
        Some(&"42".to_string())
    );
}

#[test]
fn test_nested_parameters() {
    let routes = vec![Arc::new(
        Route::new("/users/:userId", dummy)
            .children(vec![Arc::new(Route::new("posts/:postId", dummy))]),
    )];

    let stack = resolve_match_stack(&routes, "/users/42/posts/7");
    assert_eq!(stack.len(), 2);

    let parent = stack.at_depth(0).unwrap();
    assert_eq!(parent.params.get("userId"), Some(&"42".to_string()));

    let child = stack.at_depth(1).unwrap();
    // Child inherits parent params
    assert_eq!(child.params.get("userId"), Some(&"42".to_string()));
    assert_eq!(child.params.get("postId"), Some(&"7".to_string()));
}

#[test]
fn test_no_match() {
    let routes = vec![Arc::new(Route::new("/dashboard", dummy))];

    let stack = resolve_match_stack(&routes, "/nonexistent");
    assert!(stack.is_empty());
}

#[test]
fn test_index_route_fallback() {
    let routes = vec![Arc::new(Route::new("/dashboard", dummy).children(vec![
        Arc::new(Route::new("", dummy)),
        Arc::new(Route::new("settings", dummy)),
    ]))];

    // Navigate to /dashboard (no child segment) → should match index
    let stack = resolve_match_stack(&routes, "/dashboard");
    assert_eq!(stack.len(), 2);
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "");
}

#[test]
fn test_four_levels_deep() {
    let routes = vec![Arc::new(Route::new("/", dummy).children(vec![Arc::new(
        Route::new("app", dummy).children(vec![Arc::new(
            Route::new("workspace/:id", dummy).children(vec![Arc::new(Route::new(
                "project/:projectId",
                dummy,
            ))]),
        )]),
    )]))];

    let stack = resolve_match_stack(&routes, "/app/workspace/abc/project/xyz");
    assert_eq!(stack.len(), 4);
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/");
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "app");
    assert_eq!(
        stack.at_depth(2).unwrap().route.config.path,
        "workspace/:id"
    );
    assert_eq!(
        stack.at_depth(3).unwrap().route.config.path,
        "project/:projectId"
    );

    // Check param inheritance
    let leaf = stack.leaf().unwrap();
    assert_eq!(leaf.params.get("id"), Some(&"abc".to_string()));
    assert_eq!(leaf.params.get("projectId"), Some(&"xyz".to_string()));
}

#[test]
fn test_backtracking() {
    let routes = vec![
        Arc::new(Route::new("/users", dummy)),
        Arc::new(
            Route::new("/users/:id", dummy).children(vec![Arc::new(Route::new("profile", dummy))]),
        ),
    ];

    // /users → matches first route exactly
    let stack = resolve_match_stack(&routes, "/users");
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/users");

    // /users/42/profile → skips first route (no children), matches second
    let stack = resolve_match_stack(&routes, "/users/42/profile");
    assert_eq!(stack.len(), 2);
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/users/:id");
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "profile");
}

#[test]
fn test_trailing_slashes() {
    let routes = vec![Arc::new(Route::new("/dashboard", dummy))];

    let stack1 = resolve_match_stack(&routes, "/dashboard");
    let stack2 = resolve_match_stack(&routes, "/dashboard/");
    assert_eq!(stack1.len(), stack2.len());
}

#[test]
fn test_match_stack_helpers() {
    let routes = vec![Arc::new(
        Route::new("/a", dummy).children(vec![Arc::new(Route::new("b", dummy))]),
    )];

    let stack = resolve_match_stack(&routes, "/a/b");

    assert!(!stack.is_empty());
    assert_eq!(stack.len(), 2);
    assert_eq!(stack.max_depth(), Some(1));
    assert!(stack.has_depth(0));
    assert!(stack.has_depth(1));
    assert!(!stack.has_depth(2));
    assert_eq!(stack.root().unwrap().route.config.path, "/a");
    assert_eq!(stack.leaf().unwrap().route.config.path, "b");
}

// ---- depth tracking tests (PARENT_DEPTH approach) ----
//
// PARENT_DEPTH is a single thread-local Option<usize>:
// - None → next outlet is ROOT (depth 0)
// - Some(d) → next outlet is CHILD (depth d+1)
//
// enter_outlet() reads PARENT_DEPTH, computes my_depth, sets PARENT_DEPTH=Some(my_depth).
// No exit/restore needed — GPUI renders depth-first, so PARENT_DEPTH is always
// correct when child Entity<RouterOutlet>::render() runs.

#[test]
fn test_depth_tracking_basic() {
    reset_outlet_depth();
    assert_eq!(current_parent_depth(), None);

    // First enter_outlet: PARENT_DEPTH=None → ROOT → depth=0
    let d1 = enter_outlet();
    assert_eq!(d1, 0);
    assert_eq!(current_parent_depth(), Some(0));

    // Second enter_outlet: PARENT_DEPTH=Some(0) → CHILD → depth=1
    let d2 = enter_outlet();
    assert_eq!(d2, 1);
    assert_eq!(current_parent_depth(), Some(1));

    // Third: depth=2
    let d3 = enter_outlet();
    assert_eq!(d3, 2);
    assert_eq!(current_parent_depth(), Some(2));
}

// ---- Pattern 1: router_view + outlets (nested routing) ----

#[test]
fn test_pattern1_router_view_with_outlets() {
    // router_view resets to None, then enters as root
    reset_outlet_depth();

    // router_view: reset → enter → depth 0
    let d0 = enter_outlet();
    assert_eq!(d0, 0);
    assert_eq!(current_parent_depth(), Some(0));

    // outlet A inside router_view's builder
    let d1 = enter_outlet();
    assert_eq!(d1, 1);

    // outlet B inside outlet A's builder
    let d2 = enter_outlet();
    assert_eq!(d2, 2);

    assert_eq!(current_parent_depth(), Some(2));
}

// ---- Pattern 2: standalone RouterOutlet (no router_view) ----

#[test]
fn test_pattern2_nested_demo_app() {
    // Simulates NestedDemoApp: standalone outlet → DashboardLayout → AnalyticsPage
    // No router_view — outlet is root.
    reset_outlet_depth();

    // NestedDemoApp's outlet renders (PARENT_DEPTH=None → ROOT → depth=0)
    let d_root = enter_outlet();
    assert_eq!(d_root, 0);
    assert_eq!(current_parent_depth(), Some(0));

    // DashboardLayout's outlet renders (PARENT_DEPTH=Some(0) → CHILD → depth=1)
    let d_child = enter_outlet();
    assert_eq!(d_child, 1);
    assert_eq!(current_parent_depth(), Some(1));
}

// ---- Pattern 3: flat routes, single standalone outlet ----

#[test]
fn test_pattern3_transition_demo_flat() {
    reset_outlet_depth();

    let d = enter_outlet(); // PARENT_DEPTH=None → ROOT → depth=0
    assert_eq!(d, 0);
    assert_eq!(current_parent_depth(), Some(0));
}

// ---- Consecutive render passes with reset ----

#[test]
fn test_consecutive_renders_with_reset() {
    // Render pass 1: /dashboard/analytics (2 levels)
    reset_outlet_depth();
    let d0 = enter_outlet();
    assert_eq!(d0, 0);
    let d1 = enter_outlet();
    assert_eq!(d1, 1);

    // Render pass 2: reset before new render (simulates router_view or new frame)
    reset_outlet_depth();
    let d0 = enter_outlet();
    assert_eq!(d0, 0); // correctly starts from root again
    let d1 = enter_outlet();
    assert_eq!(d1, 1);
}

// ---- Navigation changes stack depth between renders ----

#[test]
fn test_navigation_changes_depth() {
    // Render 1: /dashboard/analytics (2 levels)
    reset_outlet_depth();
    let d0 = enter_outlet();
    let d1 = enter_outlet();
    assert_eq!(d0, 0);
    assert_eq!(d1, 1);

    // Navigation: push("/") — match_stack becomes 1 entry

    // Render 2: / (1 level only)
    reset_outlet_depth();
    let d0 = enter_outlet();
    assert_eq!(d0, 0);
    // HomePage has no outlets — done

    // Navigation: push("/products/3") — match_stack becomes 2 entries

    // Render 3: /products/3 (2 levels)
    reset_outlet_depth();
    let d0 = enter_outlet();
    assert_eq!(d0, 0);
    let d1 = enter_outlet();
    assert_eq!(d1, 1);
}

// ---- Deep nesting: 4 levels ----

#[test]
fn test_four_level_nesting() {
    reset_outlet_depth();

    // / → app → workspace/:id → project/:pid
    let d0 = enter_outlet();
    assert_eq!(d0, 0);
    let d1 = enter_outlet();
    assert_eq!(d1, 1);
    let d2 = enter_outlet();
    assert_eq!(d2, 2);
    let d3 = enter_outlet();
    assert_eq!(d3, 3);

    assert_eq!(current_parent_depth(), Some(3));
}

// ---- Reset between different patterns ----

#[test]
fn test_router_view_then_standalone() {
    // First: router_view pattern (reset + enter)
    reset_outlet_depth();
    let d0 = enter_outlet();
    assert_eq!(d0, 0);
    let d1 = enter_outlet();
    assert_eq!(d1, 1);

    // Second: standalone pattern — must reset first
    reset_outlet_depth();
    let d = enter_outlet();
    assert_eq!(d, 0); // correctly root
}

// ---- current_outlet_depth returns next child depth ----

#[test]
fn test_current_outlet_depth() {
    reset_outlet_depth();
    // No parent → current_outlet_depth returns 0 (root would be depth 0)
    assert_eq!(current_outlet_depth(), 0);

    enter_outlet(); // depth 0, sets PARENT_DEPTH=Some(0)
                    // Next child would be at depth 1
    assert_eq!(current_outlet_depth(), 1);

    enter_outlet(); // depth 1, sets PARENT_DEPTH=Some(1)
    assert_eq!(current_outlet_depth(), 2);
}

#[test]
fn test_index_named_route() {
    let routes = vec![Arc::new(
        Route::new("/dashboard", dummy).children(vec![Arc::new(Route::new("index", dummy))]),
    )];

    let stack = resolve_match_stack(&routes, "/dashboard");
    assert_eq!(stack.len(), 2);
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "index");
}

#[test]
fn test_multi_segment_route_path() {
    let routes = vec![Arc::new(
        Route::new("/api/v1", dummy).children(vec![Arc::new(Route::new("users", dummy))]),
    )];

    let stack = resolve_match_stack(&routes, "/api/v1/users");
    assert_eq!(stack.len(), 2);
    assert_eq!(stack.at_depth(0).unwrap().route.config.path, "/api/v1");
    assert_eq!(stack.at_depth(1).unwrap().route.config.path, "users");
}

#[test]
fn test_empty_match_stack() {
    let stack = MatchStack::new();
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
    assert!(stack.root().is_none());
    assert!(stack.leaf().is_none());
    assert!(stack.max_depth().is_none());
    assert!(stack.params().is_empty());
}
