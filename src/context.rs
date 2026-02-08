//! Router context integration for GPUI.
//!
//! This module provides the global router state management through GPUI's
//! context system. It contains three key types:
//!
//! - [`GlobalRouter`] — the central routing object stored as a GPUI `Global`.
//!   It owns the [`RouterState`](crate::RouterState), the route registry,
//!   and orchestrates the full navigation pipeline (guards → middleware →
//!   navigation → middleware).
//!
//! - [`Navigator`] — a convenience API with static methods
//!   (`Navigator::push`, `Navigator::pop`, …) that read/write the
//!   `GlobalRouter` through `cx`.
//!
//! - [`NavigatorHandle`] — returned by [`Navigator::of(cx)`](Navigator::of),
//!   enables fluent chained navigation calls.
//!
//! # Initialization
//!
//! Use [`init_router`] to set up the global router before any navigation:
//!
//! ```ignore
//! use gpui_navigator::{init_router, Route};
//!
//! init_router(cx, |router| {
//!     router.add_route(Route::new("/", |_, _cx, _params| gpui::div()));
//! });
//! ```

#[cfg(feature = "cache")]
use crate::cache::{CacheStats, RouteCache};
use crate::error::NavigationResult;
#[cfg(feature = "guard")]
use crate::lifecycle::NavigationAction;
use crate::resolve::{resolve_match_stack, MatchStack};
use crate::route::NamedRouteRegistry;
#[cfg(feature = "transition")]
use crate::transition::Transition;
use crate::{
    debug_log, error_log, info_log, trace_log, warn_log, IntoRoute, Route, RouteParams, RouterState,
};
use gpui::{AnyView, App, BorrowAppContext, Global};
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::sync::Arc;

/// Maximum redirect depth to prevent infinite redirect loops.
const MAX_REDIRECT_DEPTH: usize = 5;

// ============================================================================
// NavigationRequest
// ============================================================================

/// Request for navigation.
///
/// Contains information about the navigation being performed, passed to guards
/// and middleware so they can inspect the source and destination.
///
/// # Example
///
/// ```
/// use gpui_navigator::NavigationRequest;
///
/// let request = NavigationRequest::new("/dashboard".to_string());
/// assert_eq!(request.to, "/dashboard");
/// ```
pub struct NavigationRequest {
    /// The path we're navigating from (if any)
    pub from: Option<String>,

    /// The path we're navigating to
    pub to: String,

    /// Route parameters extracted from the path
    pub params: RouteParams,
}

impl NavigationRequest {
    /// Create a new navigation request.
    pub fn new(to: String) -> Self {
        Self {
            from: None,
            to,
            params: RouteParams::new(),
        }
    }

    /// Create a navigation request with a source path.
    pub fn with_from(to: String, from: String) -> Self {
        Self {
            from: Some(from),
            to,
            params: RouteParams::new(),
        }
    }

    /// Set route parameters.
    pub fn with_params(mut self, params: RouteParams) -> Self {
        self.params = params;
        self
    }
}

impl std::fmt::Debug for NavigationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavigationRequest")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// GlobalRouter
// ============================================================================

/// Global router state accessible from any component.
///
/// This is the central routing object stored as a GPUI global. It holds the
/// navigation state, route registry, and orchestrates the navigation pipeline
/// (guards -> middleware -> navigation -> middleware).
#[derive(Clone)]
pub struct GlobalRouter {
    state: RouterState,
    /// Pre-resolved route chain for the current path.
    /// Built once per navigation, consumed by outlets during render.
    match_stack: MatchStack,
    /// Previous match stack — used for transition exit animations.
    #[cfg(feature = "transition")]
    previous_stack: Option<MatchStack>,
    #[cfg(feature = "cache")]
    nested_cache: RouteCache,
    named_routes: NamedRouteRegistry,
    #[cfg(feature = "transition")]
    next_transition: Option<Transition>,
    /// Cache for component entities created by `Route::component()`.
    /// Unlike `window.use_keyed_state()` which is frame-scoped, this cache
    /// persists across navigations so that component state survives when the
    /// user navigates away and back.
    component_cache: HashMap<String, AnyView>,
}

impl GlobalRouter {
    /// Create a new global router with empty state and no registered routes.
    pub fn new() -> Self {
        Self {
            state: RouterState::new(),
            match_stack: MatchStack::new(),
            #[cfg(feature = "transition")]
            previous_stack: None,
            #[cfg(feature = "cache")]
            nested_cache: RouteCache::new(),
            named_routes: NamedRouteRegistry::new(),
            #[cfg(feature = "transition")]
            next_transition: None,
            component_cache: HashMap::new(),
        }
    }

    /// Get the pre-resolved match stack for the current path.
    ///
    /// Outlets call this during render to find their route by depth index.
    /// The stack is built once per navigation, so this is O(1).
    pub fn match_stack(&self) -> &MatchStack {
        &self.match_stack
    }

    /// Get the previous match stack (for transition animations).
    #[cfg(feature = "transition")]
    pub fn previous_stack(&self) -> Option<&MatchStack> {
        self.previous_stack.as_ref()
    }

    /// Re-resolve the match stack after routes change.
    fn re_resolve(&mut self) {
        self.match_stack = resolve_match_stack(self.state.routes(), self.state.current_path());
    }

    /// Register a route and re-resolve the match stack.
    ///
    /// If the route has a [`name`](crate::route::RouteConfig::name), it is
    /// also registered in the [`NamedRouteRegistry`] for URL generation via
    /// [`url_for`](Self::url_for).
    pub fn add_route(&mut self, route: Route) {
        if let Some(name) = &route.config.name {
            info_log!(
                "Registered route '{}' (name: '{}')",
                route.config.path,
                name
            );
            self.named_routes
                .register(name.clone(), route.config.path.clone());
        } else {
            info_log!("Registered route '{}'", route.config.path);
        }
        self.state.add_route(route);
        #[cfg(feature = "cache")]
        self.nested_cache.clear();
        // Re-resolve match stack after adding routes
        self.re_resolve();
    }

    // ========================================================================
    // Navigation pipeline
    // ========================================================================

    /// Navigate to a path, running the full guard/middleware pipeline.
    ///
    /// Pipeline:
    /// 1. Collect guards from matched route (+ ancestors)
    /// 2. Check guards — if any denies/redirects, navigation is blocked
    /// 3. Run `before_navigation` middleware
    /// 4. Perform actual navigation
    /// 5. Run `after_navigation` middleware
    pub fn push(&mut self, path: String, cx: &App) -> NavigationResult {
        self.navigate_with_pipeline(path, cx, NavigateOp::Push, 0)
    }

    /// Replace current path, running the full guard/middleware pipeline.
    pub fn replace(&mut self, path: String, cx: &App) -> NavigationResult {
        self.navigate_with_pipeline(path, cx, NavigateOp::Replace, 0)
    }

    /// Go back in history, checking guards on the target route.
    pub fn back(&mut self, cx: &App) -> Option<NavigationResult> {
        let target = self.state.peek_back_path()?.to_string();
        Some(self.navigate_with_pipeline(target, cx, NavigateOp::Back, 0))
    }

    /// Go forward in history, checking guards on the target route.
    pub fn forward(&mut self, cx: &App) -> Option<NavigationResult> {
        let target = self.state.peek_forward_path()?.to_string();
        Some(self.navigate_with_pipeline(target, cx, NavigateOp::Forward, 0))
    }

    /// Core navigation method that runs the full pipeline.
    fn navigate_with_pipeline(
        &mut self,
        path: String,
        cx: &App,
        op: NavigateOp,
        redirect_depth: usize,
    ) -> NavigationResult {
        if redirect_depth >= MAX_REDIRECT_DEPTH {
            error_log!(
                "Redirect loop detected (depth {}) navigating to '{}'",
                redirect_depth,
                path
            );
            return NavigationResult::Blocked {
                reason: format!(
                    "Redirect loop detected (depth {}): target '{}'",
                    redirect_depth, path
                ),
                redirect: None,
            };
        }

        let from = self.current_path().to_string();
        info_log!("Navigation {:?}: '{}' → '{}'", op, from, path);
        let request = NavigationRequest::with_from(path.clone(), from);

        // Step 1: Run guards
        #[cfg(feature = "guard")]
        {
            let guard_result = self.run_guards(cx, &request);
            match guard_result {
                NavigationAction::Continue => {}
                NavigationAction::Deny { reason } => {
                    warn_log!("Navigation to '{}' blocked: {}", path, reason);
                    return NavigationResult::Blocked {
                        reason,
                        redirect: None,
                    };
                }
                NavigationAction::Redirect { to, reason } => {
                    debug_log!(
                        "Guard redirecting from '{}' to '{}': {:?}",
                        path,
                        to,
                        reason
                    );
                    return self.navigate_with_pipeline(
                        to,
                        cx,
                        NavigateOp::Push,
                        redirect_depth + 1,
                    );
                }
            }
        }

        // Step 2: Run before middleware
        #[cfg(feature = "middleware")]
        self.run_middleware_before(cx, &request);

        // Step 3: Perform actual navigation
        #[cfg(feature = "cache")]
        self.nested_cache.clear();

        // Save previous stack for transition animations
        #[cfg(feature = "transition")]
        {
            self.previous_stack = Some(self.match_stack.clone());
        }

        let event = match op {
            NavigateOp::Push => self.state.push(path),
            NavigateOp::Replace => self.state.replace(path),
            NavigateOp::Back => {
                // We already validated peek_back_path, so unwrap is safe
                self.state.back().expect("back() should succeed after peek")
            }
            NavigateOp::Forward => self
                .state
                .forward()
                .expect("forward() should succeed after peek"),
        };

        // Resolve match stack immediately after navigation
        self.match_stack = resolve_match_stack(self.state.routes(), self.state.current_path());

        // Step 4: Run after middleware
        #[cfg(feature = "middleware")]
        self.run_middleware_after(cx, &request);

        info_log!(
            "Navigation complete: '{}' (stack depth: {})",
            event.to,
            self.match_stack.len()
        );
        NavigationResult::Success { path: event.to }
    }

    /// Collect and run guards for the target path.
    ///
    /// Walks the route tree to find the target route, collecting guards from
    /// every ancestor route along the way. Guards on parent routes also protect
    /// child routes (e.g. an `AuthGuard` on `/dashboard` also guards `/dashboard/settings`).
    #[cfg(feature = "guard")]
    fn run_guards(&self, cx: &App, request: &NavigationRequest) -> NavigationAction {
        let path = request.to.trim_start_matches('/').trim_end_matches('/');
        let mut guards: Vec<(&dyn crate::guards::RouteGuard, i32)> = Vec::new();

        // Collect guards from matching routes (including ancestor routes)
        for route in self.state.routes() {
            Self::collect_guards_recursive(route, path, "", &mut guards);
        }

        // Sort by priority (higher first)
        guards.sort_by_key(|(_, prio)| std::cmp::Reverse(*prio));

        debug_log!("Collected {} guards for '{}'", guards.len(), path);

        // Check each guard — first non-Continue result wins
        for (guard, prio) in &guards {
            let result = guard.check(cx, request);
            trace_log!(
                "Guard '{}' (priority {}) → {:?}",
                guard.name(),
                prio,
                result
            );
            if !matches!(result, NavigationAction::Continue) {
                debug_log!(
                    "Guard '{}' blocked navigation to '{}'",
                    guard.name(),
                    request.to
                );
                return result;
            }
        }

        NavigationAction::Continue
    }

    /// Recursively walk the route tree, collecting guards from routes that match
    /// the given path (as exact match or prefix).
    #[cfg(feature = "guard")]
    fn collect_guards_recursive<'a>(
        route: &'a Arc<Route>,
        path: &str,
        accumulated: &str,
        out: &mut Vec<(&'a dyn crate::guards::RouteGuard, i32)>,
    ) {
        walk_matching_routes(route, path, accumulated, &mut |r, _full| {
            for guard in &r.guards {
                out.push((guard.as_ref(), guard.priority()));
            }
        });
    }

    /// Run `before_navigation` on all middleware attached to matching routes.
    #[cfg(feature = "middleware")]
    fn run_middleware_before(&self, cx: &App, request: &NavigationRequest) {
        let path = request.to.trim_start_matches('/').trim_end_matches('/');
        let mut middleware: Vec<(&dyn crate::middleware::RouteMiddleware, i32)> = Vec::new();

        for route in self.state.routes() {
            Self::collect_middleware_recursive(route, path, "", &mut middleware);
        }

        // Sort by priority (higher first for before)
        middleware.sort_by_key(|(_, prio)| std::cmp::Reverse(*prio));

        debug_log!(
            "Running {} before-middleware for '{}'",
            middleware.len(),
            request.to
        );
        for (mw, _) in &middleware {
            trace_log!(
                "Middleware '{}' before_navigation for '{}'",
                mw.name(),
                request.to
            );
            mw.before_navigation(cx, request);
        }
    }

    /// Run `after_navigation` on all middleware attached to matching routes.
    #[cfg(feature = "middleware")]
    fn run_middleware_after(&self, cx: &App, request: &NavigationRequest) {
        let path = request.to.trim_start_matches('/').trim_end_matches('/');
        let mut middleware: Vec<(&dyn crate::middleware::RouteMiddleware, i32)> = Vec::new();

        for route in self.state.routes() {
            Self::collect_middleware_recursive(route, path, "", &mut middleware);
        }

        // Sort by priority ascending for after (reverse of before — stack-like)
        middleware.sort_by_key(|(_, prio)| *prio);

        debug_log!(
            "Running {} after-middleware for '{}'",
            middleware.len(),
            request.to
        );
        for (mw, _) in &middleware {
            trace_log!(
                "Middleware '{}' after_navigation for '{}'",
                mw.name(),
                request.to
            );
            mw.after_navigation(cx, request);
        }
    }

    /// Recursively collect middleware from matching routes.
    #[cfg(feature = "middleware")]
    fn collect_middleware_recursive<'a>(
        route: &'a Arc<Route>,
        path: &str,
        accumulated: &str,
        out: &mut Vec<(&'a dyn crate::middleware::RouteMiddleware, i32)>,
    ) {
        walk_matching_routes(route, path, accumulated, &mut |r, _full| {
            for mw in &r.middleware {
                out.push((mw.as_ref(), mw.priority()));
            }
        });
    }

    // ========================================================================
    // Named routes
    // ========================================================================

    /// Navigate to a named route, resolving the URL from `params`.
    ///
    /// Returns `None` if the name is not registered.
    pub fn push_named(
        &mut self,
        name: &str,
        params: &RouteParams,
        cx: &App,
    ) -> Option<NavigationResult> {
        let url = match self.named_routes.url_for(name, params) {
            Some(url) => {
                debug_log!("Named route '{}' resolved to '{}'", name, url);
                url
            }
            None => {
                warn_log!("Named route '{}' not found in registry", name);
                return None;
            }
        };
        Some(self.push(url, cx))
    }

    /// Generate a URL for a named route by substituting `params` into its pattern.
    ///
    /// Returns `None` if the name is not registered.
    pub fn url_for(&self, name: &str, params: &RouteParams) -> Option<String> {
        self.named_routes.url_for(name, params)
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Return the current navigation path.
    pub fn current_path(&self) -> &str {
        self.state.current_path()
    }

    /// Get current route match (with caching, requires mutable).
    pub fn current_match(&mut self) -> Option<crate::RouteMatch> {
        self.state.current_match()
    }

    /// Get current route match (immutable, no caching).
    pub fn current_match_immutable(&self) -> Option<crate::RouteMatch> {
        self.state.current_match_immutable()
    }

    /// Get the current matched Route.
    pub fn current_route(&self) -> Option<&Arc<crate::route::Route>> {
        self.state.current_route()
    }

    /// Check if can go back.
    pub fn can_go_back(&self) -> bool {
        self.state.can_go_back()
    }

    /// Check if can go forward.
    pub fn can_go_forward(&self) -> bool {
        self.state.can_go_forward()
    }

    /// Get mutable state reference.
    pub fn state_mut(&mut self) -> &mut RouterState {
        &mut self.state
    }

    /// Get state reference.
    pub fn state(&self) -> &RouterState {
        &self.state
    }

    /// Get nested route cache (mutable).
    #[cfg(feature = "cache")]
    pub fn nested_cache_mut(&mut self) -> &mut RouteCache {
        &mut self.nested_cache
    }

    /// Get nested route cache statistics.
    #[cfg(feature = "cache")]
    pub fn cache_stats(&self) -> &CacheStats {
        self.nested_cache.stats()
    }

    // ========================================================================
    // Component cache
    // ========================================================================

    /// Get a cached component view by key.
    pub fn get_cached_component(&self, key: &str) -> Option<&AnyView> {
        self.component_cache.get(key)
    }

    /// Store a component view in the cache.
    pub fn cache_component(&mut self, key: String, view: AnyView) {
        self.component_cache.insert(key, view);
    }

    // ========================================================================
    // Transitions
    // ========================================================================

    /// Set transition for the next navigation.
    #[cfg(feature = "transition")]
    pub fn set_next_transition(&mut self, transition: Transition) {
        self.next_transition = Some(transition);
    }

    /// Get and consume the next transition override.
    #[cfg(feature = "transition")]
    pub fn take_next_transition(&mut self) -> Option<Transition> {
        self.next_transition.take()
    }

    /// Check if there's a transition override set.
    #[cfg(feature = "transition")]
    pub fn has_next_transition(&self) -> bool {
        self.next_transition.is_some()
    }

    /// Clear transition override.
    #[cfg(feature = "transition")]
    pub fn clear_next_transition(&mut self) {
        self.next_transition = None;
    }

    /// Navigate with a specific transition.
    #[cfg(feature = "transition")]
    pub fn push_with_transition(
        &mut self,
        path: String,
        transition: Transition,
        cx: &App,
    ) -> NavigationResult {
        self.set_next_transition(transition);
        self.push(path, cx)
    }

    /// Replace with a specific transition.
    #[cfg(feature = "transition")]
    pub fn replace_with_transition(
        &mut self,
        path: String,
        transition: Transition,
        cx: &App,
    ) -> NavigationResult {
        self.set_next_transition(transition);
        self.replace(path, cx)
    }
}

impl Default for GlobalRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl Global for GlobalRouter {}

// ============================================================================
// Helper: path prefix matching with parameter support
// ============================================================================

/// Walk the route tree, calling `visitor` on each route whose accumulated path
/// is a prefix of `target_path`. The visitor receives the route and the full
/// accumulated path.
///
/// This factored-out helper avoids duplicating tree-walk logic between guard
/// collection and middleware collection.
fn walk_matching_routes<'a>(
    route: &'a Arc<Route>,
    target_path: &str,
    accumulated: &str,
    visitor: &mut dyn FnMut(&'a Route, &str),
) {
    let route_path = route
        .config
        .path
        .trim_start_matches('/')
        .trim_end_matches('/');

    let full = if accumulated.is_empty() {
        route_path.to_string()
    } else if route_path.is_empty() {
        accumulated.to_string()
    } else {
        format!("{}/{}", accumulated, route_path)
    };

    let matches = if full.is_empty() {
        true
    } else {
        path_matches_prefix(target_path, &full)
    };

    if !matches {
        return;
    }

    visitor(route, &full);

    for child in route.get_children() {
        walk_matching_routes(child, target_path, &full, visitor);
    }
}

/// Check if `path` matches `prefix` as a route prefix (supports `:param` segments).
///
/// Examples:
/// - `path_matches_prefix("dashboard/settings", "dashboard")` → true
/// - `path_matches_prefix("dashboard", "dashboard")` → true
/// - `path_matches_prefix("users/123", "users/:id")` → true
/// - `path_matches_prefix("other", "dashboard")` → false
fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    let path_segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let prefix_segs: Vec<&str> = prefix.split('/').filter(|s| !s.is_empty()).collect();

    if path_segs.len() < prefix_segs.len() {
        return false;
    }

    for (ps, pfs) in path_segs.iter().zip(prefix_segs.iter()) {
        if pfs.starts_with(':') {
            // Parameter segment matches anything
            continue;
        }
        if ps != pfs {
            return false;
        }
    }

    true
}

// ============================================================================
// Navigation operation type
// ============================================================================

/// Internal enum for the kind of navigation to perform after pipeline checks.
#[derive(Debug, Clone, Copy)]
enum NavigateOp {
    Push,
    Replace,
    Back,
    Forward,
}

// ============================================================================
// UseRouter trait
// ============================================================================

/// Trait for accessing the global router from context.
pub trait UseRouter {
    /// Get reference to global router.
    fn router(&self) -> &GlobalRouter;

    /// Update global router.
    fn update_router<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut GlobalRouter, &mut App) -> R;
}

impl UseRouter for App {
    fn router(&self) -> &GlobalRouter {
        self.global::<GlobalRouter>()
    }

    fn update_router<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut GlobalRouter, &mut App) -> R,
    {
        self.update_global(f)
    }
}

// ============================================================================
// init_router
// ============================================================================

/// Initialize global router with routes.
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::{init_router, Route};
///
/// init_router(cx, |router| {
///     router.add_route(Route::new("/", |_, _cx, _params| gpui::div()));
///     router.add_route(Route::new("/users/:id", |_, _cx, _params| gpui::div()));
/// });
/// ```
pub fn init_router<F>(cx: &mut App, configure: F)
where
    F: FnOnce(&mut GlobalRouter),
{
    let mut router = GlobalRouter::new();
    configure(&mut router);
    cx.set_global(router);
}

/// Navigate to a path using the global router and refresh all windows.
///
/// This is a convenience shortcut equivalent to
/// `cx.update_global::<GlobalRouter, _>(|r, cx| r.push(path, cx))`.
pub fn navigate(cx: &mut App, path: impl Into<String>) {
    let path = path.into();
    cx.update_global::<GlobalRouter, _>(|router, cx| {
        router.push(path, cx);
    });
    cx.refresh_windows();
}

/// Return the current path from the global router.
pub fn current_path(cx: &App) -> String {
    cx.router().current_path().to_string()
}

// ============================================================================
// NavigatorHandle
// ============================================================================

/// Handle returned by [`Navigator::of`] for fluent chained navigation.
///
/// Each method consumes and returns `self`, allowing patterns like:
///
/// ```ignore
/// Navigator::of(cx)
///     .push("/users")
///     .push("/users/42");
/// ```
pub struct NavigatorHandle<'a, C: BorrowAppContext> {
    cx: &'a mut C,
}

impl<C: BorrowAppContext + BorrowMut<App>> NavigatorHandle<'_, C> {
    /// Navigate to a new path.
    pub fn push(self, route: impl IntoRoute) -> Self {
        let descriptor = route.into_route();
        self.cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.push(descriptor.path, app);
        });
        self.cx.borrow_mut().refresh_windows();
        self
    }

    /// Replace current path without adding to history.
    pub fn replace(self, route: impl IntoRoute) -> Self {
        let descriptor = route.into_route();
        self.cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.replace(descriptor.path, app);
        });
        self.cx.borrow_mut().refresh_windows();
        self
    }

    /// Go back to the previous route.
    pub fn pop(self) -> Self {
        self.cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.back(app);
        });
        self.cx.borrow_mut().refresh_windows();
        self
    }

    /// Go forward in history.
    pub fn forward(self) -> Self {
        self.cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.forward(app);
        });
        self.cx.borrow_mut().refresh_windows();
        self
    }
}

// ============================================================================
// Navigator
// ============================================================================

/// Navigation API for convenient route navigation.
///
/// Provides static methods for navigation operations:
/// - `Navigator::push(cx, "/path")` — Navigate to a new page
/// - `Navigator::pop(cx)` — Go back to previous page
/// - `Navigator::replace(cx, "/path")` — Replace current page
///
/// All navigation methods run the full pipeline (guards, middleware).
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::Navigator;
///
/// Navigator::push(cx, "/users/123");
/// Navigator::pop(cx);
/// Navigator::replace(cx, "/login");
/// ```
pub struct Navigator;

impl Navigator {
    /// Get a [`NavigatorHandle`] for chained navigation calls.
    pub fn of<C: BorrowAppContext + BorrowMut<App>>(cx: &mut C) -> NavigatorHandle<'_, C> {
        NavigatorHandle { cx }
    }

    /// Navigate to a new path.
    pub fn push(cx: &mut (impl BorrowAppContext + BorrowMut<App>), route: impl IntoRoute) {
        let descriptor = route.into_route();
        debug_log!("Navigator::push: pushing path '{}'", descriptor.path);
        cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.push(descriptor.path, app);
        });
        cx.borrow_mut().refresh_windows();
    }

    /// Replace current path without adding to history.
    pub fn replace(cx: &mut (impl BorrowAppContext + BorrowMut<App>), route: impl IntoRoute) {
        let descriptor = route.into_route();
        cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.replace(descriptor.path, app);
        });
        cx.borrow_mut().refresh_windows();
    }

    /// Go back to the previous route.
    pub fn pop(cx: &mut (impl BorrowAppContext + BorrowMut<App>)) {
        cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.back(app);
        });
        cx.borrow_mut().refresh_windows();
    }

    /// Alias for [`pop`](Navigator::pop).
    pub fn back(cx: &mut (impl BorrowAppContext + BorrowMut<App>)) {
        Self::pop(cx);
    }

    /// Go forward in history.
    pub fn forward(cx: &mut (impl BorrowAppContext + BorrowMut<App>)) {
        cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.forward(app);
        });
        cx.borrow_mut().refresh_windows();
    }

    /// Get current path.
    pub fn current_path(cx: &App) -> String {
        cx.global::<GlobalRouter>().current_path().to_string()
    }

    /// Check if can go back.
    pub fn can_pop(cx: &App) -> bool {
        cx.global::<GlobalRouter>().can_go_back()
    }

    /// Alias for [`can_pop`](Navigator::can_pop).
    pub fn can_go_back(cx: &App) -> bool {
        Self::can_pop(cx)
    }

    /// Check if can go forward.
    pub fn can_go_forward(cx: &App) -> bool {
        cx.global::<GlobalRouter>().can_go_forward()
    }

    /// Navigate to a named route with parameters.
    pub fn push_named(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        name: &str,
        params: &RouteParams,
    ) {
        let name = name.to_string();
        let params = params.clone();
        cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.push_named(&name, &params, app);
        });
        cx.borrow_mut().refresh_windows();
    }

    /// Generate URL for a named route.
    pub fn url_for(cx: &App, name: &str, params: &RouteParams) -> Option<String> {
        cx.global::<GlobalRouter>().url_for(name, params)
    }

    /// Set transition for the next navigation.
    #[cfg(feature = "transition")]
    pub fn set_next_transition(cx: &mut impl BorrowAppContext, transition: Transition) {
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.set_next_transition(transition);
        });
    }

    /// Navigate with a specific transition.
    #[cfg(feature = "transition")]
    pub fn push_with_transition(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        route: impl IntoRoute,
        transition: Transition,
    ) {
        let descriptor = route.into_route();
        cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.push_with_transition(descriptor.path, transition, app);
        });
        cx.borrow_mut().refresh_windows();
    }

    /// Replace with a specific transition.
    #[cfg(feature = "transition")]
    pub fn replace_with_transition(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        route: impl IntoRoute,
        transition: Transition,
    ) {
        let descriptor = route.into_route();
        cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.replace_with_transition(descriptor.path, transition, app);
        });
        cx.borrow_mut().refresh_windows();
    }

    /// Push named route with a specific transition.
    #[cfg(feature = "transition")]
    pub fn push_named_with_transition(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        name: &str,
        params: &RouteParams,
        transition: Transition,
    ) {
        let name = name.to_string();
        let params = params.clone();
        cx.update_global::<GlobalRouter, _>(|router, cx| {
            let app: &App = cx.borrow_mut();
            router.set_next_transition(transition);
            router.push_named(&name, &params, app);
        });
        cx.borrow_mut().refresh_windows();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{IntoElement, TestAppContext};

    #[gpui::test]
    fn test_nav_push(cx: &mut TestAppContext) {
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/users", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/users/:id", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        let initial_path = cx.read(Navigator::current_path);
        assert_eq!(initial_path, "/");

        cx.update(|cx| Navigator::push(cx, "/users"));
        assert_eq!(cx.read(Navigator::current_path), "/users");

        cx.update(|cx| Navigator::push(cx, "/users/123"));
        assert_eq!(cx.read(Navigator::current_path), "/users/123");
    }

    #[gpui::test]
    fn test_nav_back_forward(cx: &mut TestAppContext) {
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/page1", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/page2", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        cx.update(|cx| {
            Navigator::push(cx, "/page1");
            Navigator::push(cx, "/page2");
        });

        assert_eq!(cx.read(Navigator::current_path), "/page2");
        assert!(cx.read(Navigator::can_pop));

        cx.update(|cx| Navigator::pop(cx));
        assert_eq!(cx.read(Navigator::current_path), "/page1");
        assert!(cx.read(Navigator::can_pop));
        assert!(cx.read(Navigator::can_go_forward));

        cx.update(|cx| Navigator::forward(cx));
        assert_eq!(cx.read(Navigator::current_path), "/page2");
        assert!(!cx.read(Navigator::can_go_forward));
    }

    #[gpui::test]
    fn test_nav_replace(cx: &mut TestAppContext) {
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/login", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/home", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        cx.update(|cx| {
            Navigator::push(cx, "/login");
            Navigator::replace(cx, "/home");
        });

        assert_eq!(cx.read(Navigator::current_path), "/home");

        cx.update(|cx| Navigator::pop(cx));
        assert_eq!(cx.read(Navigator::current_path), "/");
    }

    #[gpui::test]
    fn test_nav_can_go_back_boundaries(cx: &mut TestAppContext) {
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        assert!(!cx.read(Navigator::can_pop));

        cx.update(|cx| Navigator::push(cx, "/page1"));
        assert!(cx.read(Navigator::can_pop));

        cx.update(|cx| Navigator::pop(cx));
        assert!(!cx.read(Navigator::can_pop));
    }

    #[gpui::test]
    fn test_nav_multiple_pushes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/step1", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/step2", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/step3", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        cx.update(|cx| {
            Navigator::push(cx, "/step1");
            Navigator::push(cx, "/step2");
            Navigator::push(cx, "/step3");
        });

        assert_eq!(cx.read(Navigator::current_path), "/step3");

        cx.update(|cx| Navigator::pop(cx));
        assert_eq!(cx.read(Navigator::current_path), "/step2");

        cx.update(|cx| Navigator::pop(cx));
        assert_eq!(cx.read(Navigator::current_path), "/step1");

        cx.update(|cx| Navigator::pop(cx));
        assert_eq!(cx.read(Navigator::current_path), "/");
    }

    #[gpui::test]
    fn test_nav_with_route_parameters(cx: &mut TestAppContext) {
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/users/:id", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new(
                    "/posts/:id/comments/:commentId",
                    |_, _cx, _params| gpui::div().into_any_element(),
                ));
            });
        });

        cx.update(|cx| Navigator::push(cx, "/users/42"));
        assert_eq!(cx.read(Navigator::current_path), "/users/42");

        cx.update(|cx| Navigator::push(cx, "/posts/123/comments/456"));
        assert_eq!(cx.read(Navigator::current_path), "/posts/123/comments/456");
    }

    #[gpui::test]
    fn test_navigator_of_style(cx: &mut TestAppContext) {
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/home", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/profile", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        cx.update(|cx| {
            Navigator::of(cx).push("/home");
        });
        assert_eq!(cx.read(Navigator::current_path), "/home");

        cx.update(|cx| {
            Navigator::of(cx).push("/profile").pop();
        });
        assert_eq!(cx.read(Navigator::current_path), "/home");

        cx.update(|cx| {
            Navigator::of(cx).replace("/profile");
        });
        assert_eq!(cx.read(Navigator::current_path), "/profile");

        assert!(cx.read(Navigator::can_pop));
        cx.update(|cx| {
            Navigator::of(cx).pop();
        });
        assert_eq!(cx.read(Navigator::current_path), "/");
        assert!(!cx.read(Navigator::can_pop));
    }

    #[gpui::test]
    fn test_string_into_route(cx: &mut TestAppContext) {
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/home", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        cx.update(|cx| Navigator::push(cx, "/home"));
        assert_eq!(cx.read(Navigator::current_path), "/home");

        cx.update(|cx| Navigator::push(cx, String::from("/home")));
        assert_eq!(cx.read(Navigator::current_path), "/home");
    }

    // ========================================================================
    // Guard integration tests
    // ========================================================================

    #[gpui::test]
    #[cfg(feature = "guard")]
    fn test_guard_blocks_navigation(cx: &mut TestAppContext) {
        use crate::AuthGuard;

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(
                    Route::new("/protected", |_, _cx, _params| {
                        gpui::div().into_any_element()
                    })
                    .guard(AuthGuard::new(|_| false, "/login")),
                );
                router.add_route(Route::new("/login", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        // Guard should redirect to /login
        cx.update(|cx| Navigator::push(cx, "/protected"));

        // We end up at /login (redirect), not /protected
        assert_eq!(cx.read(Navigator::current_path), "/login");
    }

    #[gpui::test]
    #[cfg(feature = "guard")]
    fn test_guard_allows_navigation(cx: &mut TestAppContext) {
        use crate::AuthGuard;

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(
                    Route::new("/dashboard", |_, _cx, _params| {
                        gpui::div().into_any_element()
                    })
                    .guard(AuthGuard::new(|_| true, "/login")),
                );
            });
        });

        cx.update(|cx| Navigator::push(cx, "/dashboard"));
        assert_eq!(cx.read(Navigator::current_path), "/dashboard");
    }

    #[gpui::test]
    #[cfg(feature = "guard")]
    fn test_guard_denies_navigation(cx: &mut TestAppContext) {
        use crate::guard_fn;

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(
                    Route::new("/forbidden", |_, _cx, _params| {
                        gpui::div().into_any_element()
                    })
                    .guard(guard_fn(|_, _| NavigationAction::deny("No access"))),
                );
            });
        });

        cx.update(|cx| Navigator::push(cx, "/forbidden"));
        // Navigation was blocked, path should remain at "/"
        assert_eq!(cx.read(Navigator::current_path), "/");
    }

    #[gpui::test]
    #[cfg(feature = "guard")]
    fn test_parent_guard_blocks_child(cx: &mut TestAppContext) {
        use crate::AuthGuard;

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(
                    Route::new("/dashboard", |_, _cx, _params| {
                        gpui::div().into_any_element()
                    })
                    .guard(AuthGuard::new(|_| false, "/login"))
                    .child(
                        Route::new("settings", |_, _cx, _params| gpui::div().into_any_element())
                            .into(),
                    ),
                );
                router.add_route(Route::new("/login", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        // Guard on /dashboard should also block /dashboard/settings
        cx.update(|cx| Navigator::push(cx, "/dashboard/settings"));
        assert_eq!(cx.read(Navigator::current_path), "/login");
    }

    #[gpui::test]
    #[cfg(feature = "guard")]
    fn test_redirect_loop_protection(cx: &mut TestAppContext) {
        use crate::guard_fn;

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                // /a redirects to /b, /b redirects to /a — infinite loop
                router.add_route(
                    Route::new("/a", |_, _cx, _params| gpui::div().into_any_element())
                        .guard(guard_fn(|_, _| NavigationAction::redirect("/b"))),
                );
                router.add_route(
                    Route::new("/b", |_, _cx, _params| gpui::div().into_any_element())
                        .guard(guard_fn(|_, _| NavigationAction::redirect("/a"))),
                );
            });
        });

        // Should not infinite loop — stays at "/"
        cx.update(|cx| Navigator::push(cx, "/a"));
        // Path stays at "/" because the redirect loop is detected and blocked
        assert_eq!(cx.read(Navigator::current_path), "/");
    }

    // ========================================================================
    // Middleware integration tests
    // ========================================================================

    #[gpui::test]
    #[cfg(feature = "middleware")]
    fn test_middleware_runs_during_navigation(cx: &mut TestAppContext) {
        use crate::middleware_fn;
        use std::sync::{Arc, Mutex};

        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let before_calls = calls.clone();
        let after_calls = calls.clone();

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(
                    Route::new("/page", |_, _cx, _params| gpui::div().into_any_element())
                        .middleware(middleware_fn(
                            move |_cx, req| {
                                before_calls
                                    .lock()
                                    .unwrap()
                                    .push(format!("before:{}", req.to));
                            },
                            move |_cx, req| {
                                after_calls
                                    .lock()
                                    .unwrap()
                                    .push(format!("after:{}", req.to));
                            },
                        )),
                );
            });
        });

        cx.update(|cx| Navigator::push(cx, "/page"));
        assert_eq!(cx.read(Navigator::current_path), "/page");

        let log = calls.lock().unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0], "before:/page");
        assert_eq!(log[1], "after:/page");
    }

    // ========================================================================
    // path_matches_prefix unit tests
    // ========================================================================

    #[test]
    fn test_path_matches_prefix_exact() {
        assert!(path_matches_prefix("dashboard", "dashboard"));
    }

    #[test]
    fn test_path_matches_prefix_child() {
        assert!(path_matches_prefix("dashboard/settings", "dashboard"));
    }

    #[test]
    fn test_path_matches_prefix_no_match() {
        assert!(!path_matches_prefix("other", "dashboard"));
    }

    #[test]
    fn test_path_matches_prefix_with_param() {
        assert!(path_matches_prefix("users/123", "users/:id"));
        assert!(path_matches_prefix("users/123/posts", "users/:id"));
    }

    #[test]
    fn test_path_matches_prefix_shorter_path() {
        assert!(!path_matches_prefix("users", "users/123"));
    }
}
