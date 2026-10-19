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
use crate::error::{ErrorHandlers, NavigationResult};
use crate::history::{HistoryEntry, HistoryState};
use crate::lifecycle::NavigationAction;
use crate::resolve::{resolve_match_stack, MatchStack};
use crate::route::NamedRouteRegistry;
#[cfg(feature = "transition")]
use crate::transition::Transition;
use crate::{
    debug_log, error_log, info_log, trace_log, warn_log, IntoRoute, QueryParams, Route, RouteParams,
    RouterState,
};
use gpui::{AnyView, App, BorrowAppContext, Global};
use std::borrow::BorrowMut;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// Maximum redirect depth to prevent infinite redirect loops.
const MAX_REDIRECT_DEPTH: usize = 5;

/// Default number of cached component views before LRU eviction kicks in.
pub const DEFAULT_COMPONENT_CACHE_CAPACITY: usize = 128;

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
#[must_use]
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
    ///
    /// Capped at `component_cache_capacity` entries; the least recently
    /// rendered entry is evicted when the limit is reached.
    component_cache: HashMap<String, CachedComponent>,
    component_cache_capacity: usize,
    /// Logical clock for LRU bookkeeping of `component_cache`.
    cache_clock: Cell<u64>,
    /// Custom error handlers for 404 and navigation errors.
    error_handlers: ErrorHandlers,
    /// Navigation waiting on async guards.
    pending: Option<PendingNavigation>,
    /// Id source for pending navigations.
    nav_seq: u64,
    /// Incremented on every completed navigation.
    change_seq: u64,
    /// The last completed navigation.
    last_change: Option<crate::RouteChangeEvent>,
    /// Flip slide transitions when navigating back.
    #[cfg(feature = "transition")]
    reverse_transitions_on_back: bool,
}

impl GlobalRouter {
    /// Create a new global router with empty state and no registered routes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the pre-resolved match stack for the current path.
    ///
    /// Outlets call this during render to find their route by depth index.
    /// The stack is built once per navigation, so this is O(1).
    #[must_use]
    pub const fn match_stack(&self) -> &MatchStack {
        &self.match_stack
    }

    /// Get the previous match stack (for transition animations).
    #[cfg(feature = "transition")]
    #[must_use]
    pub const fn previous_stack(&self) -> Option<&MatchStack> {
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
        self.navigate_with_pipeline(NavIntent::new(path, NavigateOp::Push), cx, 0)
    }

    /// Replace current path, running the full guard/middleware pipeline.
    pub fn replace(&mut self, path: String, cx: &App) -> NavigationResult {
        self.navigate_with_pipeline(NavIntent::new(path, NavigateOp::Replace), cx, 0)
    }

    /// Go back in history, checking guards on the target route.
    pub fn back(&mut self, cx: &App) -> Option<NavigationResult> {
        let target = self.state.peek_back_path()?.to_string();
        Some(self.navigate_with_pipeline(NavIntent::new(target, NavigateOp::Back), cx, 0))
    }

    /// Go forward in history, checking guards on the target route.
    pub fn forward(&mut self, cx: &App) -> Option<NavigationResult> {
        let target = self.state.peek_forward_path()?.to_string();
        Some(self.navigate_with_pipeline(NavIntent::new(target, NavigateOp::Forward), cx, 0))
    }

    /// Push a new path with associated [`HistoryState`] data, running the full pipeline.
    ///
    /// Allows attaching arbitrary state (scroll position, form data, typed values)
    /// to the history entry. The state is only stored if the navigation lands on
    /// `path`; it is dropped when a guard redirects elsewhere.
    pub fn push_with_state(
        &mut self,
        path: String,
        state: HistoryState,
        cx: &App,
    ) -> NavigationResult {
        let mut intent = NavIntent::new(path, NavigateOp::Push);
        intent.state = Some(state);
        self.navigate_with_pipeline(intent, cx, 0)
    }

    /// Replace current path with associated [`HistoryState`] data, running the full pipeline.
    pub fn replace_with_state(
        &mut self,
        path: String,
        state: HistoryState,
        cx: &App,
    ) -> NavigationResult {
        let mut intent = NavIntent::new(path, NavigateOp::Replace);
        intent.state = Some(state);
        self.navigate_with_pipeline(intent, cx, 0)
    }

    /// Return the current [`HistoryEntry`] (path + optional state data).
    #[must_use]
    pub fn current_entry(&self) -> &HistoryEntry {
        self.state.current_entry()
    }

    /// Core navigation method that runs the full pipeline.
    ///
    /// Pipeline:
    /// 1. Resolve the target path into a match stack
    /// 2. Guards of every matched level (sync, then async → `Pending`)
    /// 3. `can_deactivate` on every level being left (leaf first)
    /// 4. `on_exit` on every level being left (leaf first)
    /// 5. `before_navigation` middleware of the target levels
    /// 6. History update + match stack swap
    /// 7. `on_enter` on every level being entered (root first); a deny rolls
    ///    the navigation back
    /// 8. `after_navigation` middleware of the target levels
    #[allow(clippy::too_many_lines)]
    fn navigate_with_pipeline(
        &mut self,
        intent: NavIntent,
        cx: &App,
        redirect_depth: usize,
    ) -> NavigationResult {
        let path = intent.path.clone();
        if redirect_depth >= MAX_REDIRECT_DEPTH {
            error_log!(
                "Redirect loop detected (depth {}) navigating to '{}'",
                redirect_depth,
                path
            );
            return NavigationResult::Blocked {
                reason: format!("Redirect loop detected (depth {redirect_depth}): target '{path}'"),
                redirect: None,
            };
        }

        // Any new navigation supersedes one that is waiting on async guards.
        if !intent.resumed {
            self.cancel_pending();
        }

        let from = self.current_path().to_string();
        info_log!("Navigation {:?}: '{}' → '{}'", intent.op, from, path);

        let target_stack = resolve_match_stack(self.state.routes(), &path);

        // Build request — used by guards, lifecycle hooks, and middleware
        let request =
            NavigationRequest::with_from(path.clone(), from.clone()).with_params(target_stack.params());

        // Step 1: Run guards
        #[cfg(feature = "guard")]
        {
            match Self::run_guards(&target_stack, cx, &request) {
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
                        intent.redirect(to),
                        cx,
                        redirect_depth + 1,
                    );
                }
            }

            if !intent.resumed {
                let futures = Self::collect_async_guards(&target_stack, cx, &request);
                if !futures.is_empty() {
                    debug_log!(
                        "Navigation to '{}' waits on {} async guard(s)",
                        path,
                        futures.len()
                    );
                    self.nav_seq += 1;
                    self.pending = Some(PendingNavigation {
                        id: self.nav_seq,
                        intent,
                        redirect_depth,
                        future: Rc::new(RefCell::new(Some(
                            crate::guards::sequence_guard_futures(futures),
                        ))),
                    });
                    return NavigationResult::Pending { path };
                }
            }
        }

        // Levels that stay mounted are shared; everything below them is left/entered.
        let common = common_prefix_len(&self.match_stack, &target_stack);

        // Step 2: Check if the levels being left allow deactivation (leaf first)
        match self.run_lifecycle_can_deactivate(common, cx) {
            NavigationAction::Continue => {}
            NavigationAction::Deny { reason } => {
                warn_log!(
                    "Lifecycle can_deactivate blocked leaving '{}': {}",
                    from,
                    reason
                );
                return NavigationResult::Blocked {
                    reason,
                    redirect: None,
                };
            }
            NavigationAction::Redirect { to, .. } => {
                return self.navigate_with_pipeline(intent.redirect(to), cx, redirect_depth + 1);
            }
        }

        // Step 3: Run on_exit on the levels being left (leaf first)
        if let NavigationAction::Deny { reason } = self.run_lifecycle_on_exit(common, cx) {
            warn_log!("Lifecycle on_exit blocked leaving '{}': {}", from, reason);
            return NavigationResult::Blocked {
                reason,
                redirect: None,
            };
        }

        // Step 4: Run before middleware
        #[cfg(feature = "middleware")]
        Self::run_middleware_before(&target_stack, cx, &request);

        // Snapshot for rollback, only when an on_enter hook could deny
        let entering_has_hooks = target_stack.entries()[common..]
            .iter()
            .any(|entry| entry.route.lifecycle.is_some());
        let snapshot = entering_has_hooks.then(|| self.snapshot());

        let release_after = self.departing_non_keep_alive(common);

        // Step 5: Perform actual navigation + swap in the resolved match stack
        let event = match self.perform_navigation(&intent, target_stack) {
            Ok(event) => event,
            Err(result) => return result,
        };

        // Step 6: Run on_enter on the levels being entered (root first)
        match self.run_lifecycle_on_enter(common, cx, &request) {
            NavigationAction::Continue => {}
            NavigationAction::Deny { reason } => {
                warn_log!(
                    "Lifecycle on_enter denied entry to '{}': {}",
                    event.to,
                    reason
                );
                if let Some(snapshot) = snapshot {
                    self.restore(snapshot);
                }
                return NavigationResult::Blocked {
                    reason,
                    redirect: None,
                };
            }
            NavigationAction::Redirect { to, .. } => {
                return self.navigate_with_pipeline(
                    NavIntent::new(to, NavigateOp::Replace),
                    cx,
                    redirect_depth + 1,
                );
            }
        }

        // Step 7: Run after middleware
        #[cfg(feature = "middleware")]
        Self::run_middleware_after(&self.match_stack, cx, &request);

        for route_path in release_after {
            let dropped = self.evict_components(&route_path);
            if dropped > 0 {
                debug_log!("Released {} component(s) of '{}'", dropped, route_path);
            }
        }
        self.change_seq += 1;
        self.last_change = Some(event.clone());

        info_log!(
            "Navigation complete: '{}' (stack depth: {})",
            event.to,
            self.match_stack.len()
        );
        NavigationResult::Success { path: event.to }
    }

    // ========================================================================
    // Navigation execution
    // ========================================================================

    /// Perform the actual history mutation, cache clear, and match stack swap.
    ///
    /// Returns `Ok(RouteChangeEvent)` on success, `Err(NavigationResult)` if the
    /// history operation fails unexpectedly.
    fn perform_navigation(
        &mut self,
        intent: &NavIntent,
        target_stack: MatchStack,
    ) -> Result<crate::RouteChangeEvent, NavigationResult> {
        #[cfg(feature = "cache")]
        self.nested_cache.clear();

        #[cfg(feature = "transition")]
        {
            self.previous_stack = Some(self.match_stack.clone());
        }

        let path = intent.path.clone();
        let event = match (intent.op, intent.state.clone()) {
            (NavigateOp::Push, None) => self.state.push(path),
            (NavigateOp::Push, Some(state)) => self.state.push_with_state(path, state),
            (NavigateOp::Replace, None) => self.state.replace(path),
            (NavigateOp::Replace, Some(state)) => self.state.replace_with_state(path, state),
            (NavigateOp::Back, _) => self.state.back().ok_or_else(|| {
                error_log!("back() returned None after peek succeeded");
                NavigationResult::Error(crate::error::NavigationError::NavigationFailed {
                    message: "History back failed unexpectedly".into(),
                })
            })?,
            (NavigateOp::Forward, _) => self.state.forward().ok_or_else(|| {
                error_log!("forward() returned None after peek succeeded");
                NavigationResult::Error(crate::error::NavigationError::NavigationFailed {
                    message: "History forward failed unexpectedly".into(),
                })
            })?,
        };

        self.match_stack = target_stack;
        Ok(event)
    }

    fn snapshot(&self) -> RouterSnapshot {
        RouterSnapshot {
            state: self.state.clone(),
            match_stack: self.match_stack.clone(),
            #[cfg(feature = "transition")]
            previous_stack: self.previous_stack.clone(),
        }
    }

    fn restore(&mut self, snapshot: RouterSnapshot) {
        self.state = snapshot.state;
        self.match_stack = snapshot.match_stack;
        #[cfg(feature = "transition")]
        {
            self.previous_stack = snapshot.previous_stack;
        }
    }

    // ========================================================================
    // Async guards
    // ========================================================================

    /// Collect the async parts of the target levels' guards (highest priority first).
    #[cfg(feature = "guard")]
    fn collect_async_guards(
        target_stack: &MatchStack,
        cx: &App,
        request: &NavigationRequest,
    ) -> Vec<crate::guards::GuardFuture> {
        let mut guards: Vec<&dyn crate::guards::RouteGuard> = target_stack
            .entries()
            .iter()
            .flat_map(|entry| entry.route.guards.iter().map(AsRef::as_ref))
            .collect();
        guards.sort_by_key(|guard| std::cmp::Reverse(guard.priority()));
        guards
            .into_iter()
            .filter_map(|guard| guard.check_async(cx, request))
            .collect()
    }

    /// Drop a navigation that is waiting on async guards.
    fn cancel_pending(&mut self) {
        if let Some(pending) = self.pending.take() {
            debug_log!(
                "Cancelling pending navigation to '{}'",
                pending.intent.path
            );
        }
    }

    /// Path of the navigation currently waiting on async guards, if any.
    #[must_use]
    pub fn pending_path(&self) -> Option<&str> {
        self.pending.as_ref().map(|p| p.intent.path.as_str())
    }

    /// Return `true` while a navigation is waiting on async guards.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Take the future of the pending navigation, to be driven by the caller.
    fn take_pending_future(&self) -> Option<(u64, crate::guards::GuardFuture)> {
        let pending = self.pending.as_ref()?;
        let future = RefCell::borrow_mut(&pending.future).take()?;
        Some((pending.id, future))
    }

    /// Continue a pending navigation once its async guards resolved.
    ///
    /// Returns `None` if the navigation was superseded in the meantime.
    fn resume_pending(
        &mut self,
        id: u64,
        action: NavigationAction,
        cx: &App,
    ) -> Option<NavigationResult> {
        if self.pending.as_ref().is_none_or(|p| p.id != id) {
            debug_log!("Async guard result for superseded navigation ignored");
            return None;
        }
        let pending = self.pending.take()?;
        let result = match action {
            NavigationAction::Continue => {
                let mut intent = pending.intent;
                intent.resumed = true;
                self.navigate_with_pipeline(intent, cx, pending.redirect_depth)
            }
            NavigationAction::Deny { reason } => {
                warn_log!(
                    "Async guard blocked navigation to '{}': {}",
                    pending.intent.path,
                    reason
                );
                NavigationResult::Blocked {
                    reason,
                    redirect: None,
                }
            }
            NavigationAction::Redirect { to, .. } => self.navigate_with_pipeline(
                pending.intent.redirect(to),
                cx,
                pending.redirect_depth + 1,
            ),
        };
        Some(result)
    }

    // ========================================================================
    // Lifecycle hooks
    // ========================================================================

    /// Run `can_deactivate` on every level being left, leaf first.
    fn run_lifecycle_can_deactivate(&self, common: usize, cx: &App) -> NavigationAction {
        for entry in self.match_stack.entries()[common..].iter().rev() {
            if let Some(ref lifecycle) = entry.route.lifecycle {
                let page = self.page_view(entry);
                let action = lifecycle.can_deactivate_page(cx, page);
                if !matches!(action, NavigationAction::Continue) {
                    return action;
                }
            }
        }
        NavigationAction::Continue
    }

    /// Run `on_exit` on every level being left, leaf first.
    fn run_lifecycle_on_exit(&self, common: usize, cx: &App) -> NavigationAction {
        for entry in self.match_stack.entries()[common..].iter().rev() {
            if let Some(ref lifecycle) = entry.route.lifecycle {
                let action = lifecycle.on_exit(cx);
                if !matches!(action, NavigationAction::Continue) {
                    return action;
                }
            }
        }
        NavigationAction::Continue
    }

    /// Run `on_enter` on every level being entered, root first.
    fn run_lifecycle_on_enter(
        &self,
        common: usize,
        cx: &App,
        request: &NavigationRequest,
    ) -> NavigationAction {
        for entry in &self.match_stack.entries()[common..] {
            if let Some(ref lifecycle) = entry.route.lifecycle {
                let action = lifecycle.on_enter(cx, request);
                if !matches!(action, NavigationAction::Continue) {
                    return action;
                }
            }
        }
        NavigationAction::Continue
    }

    /// Collect and run guards for the target path.
    ///
    /// Guards are taken from every level of the target's match stack, so a
    /// guard on a parent route also protects its children (e.g. an `AuthGuard`
    /// on `/dashboard` also guards `/dashboard/settings`), and only routes that
    /// actually match contribute guards.
    #[cfg(feature = "guard")]
    fn run_guards(
        target_stack: &MatchStack,
        cx: &App,
        request: &NavigationRequest,
    ) -> NavigationAction {
        let mut guards: Vec<&dyn crate::guards::RouteGuard> = target_stack
            .entries()
            .iter()
            .flat_map(|entry| entry.route.guards.iter().map(AsRef::as_ref))
            .collect();

        // Sort by priority (higher first)
        guards.sort_by_key(|guard| std::cmp::Reverse(guard.priority()));

        debug_log!("Collected {} guards for '{}'", guards.len(), request.to);

        // Check each guard — first non-Continue result wins
        for guard in guards {
            let result = guard.check(cx, request);
            trace_log!(
                "Guard '{}' (priority {}) → {:?}",
                guard.name(),
                guard.priority(),
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

    /// Middleware of every level of `stack`, with their priorities.
    #[cfg(feature = "middleware")]
    fn collect_middleware(stack: &MatchStack) -> Vec<&dyn crate::middleware::RouteMiddleware> {
        stack
            .entries()
            .iter()
            .flat_map(|entry| entry.route.middleware.iter().map(AsRef::as_ref))
            .collect()
    }

    /// Run `before_navigation` on all middleware attached to the target levels.
    #[cfg(feature = "middleware")]
    fn run_middleware_before(target_stack: &MatchStack, cx: &App, request: &NavigationRequest) {
        let mut middleware = Self::collect_middleware(target_stack);

        // Sort by priority (higher first for before)
        middleware.sort_by_key(|mw| std::cmp::Reverse(mw.priority()));

        debug_log!(
            "Running {} before-middleware for '{}'",
            middleware.len(),
            request.to
        );
        for mw in middleware {
            trace_log!(
                "Middleware '{}' before_navigation for '{}'",
                mw.name(),
                request.to
            );
            mw.before_navigation(cx, request);
        }
    }

    /// Run `after_navigation` on all middleware attached to the target levels.
    #[cfg(feature = "middleware")]
    fn run_middleware_after(target_stack: &MatchStack, cx: &App, request: &NavigationRequest) {
        let mut middleware = Self::collect_middleware(target_stack);

        // Sort by priority ascending for after (reverse of before — stack-like)
        middleware.sort_by_key(|mw| mw.priority());

        debug_log!(
            "Running {} after-middleware for '{}'",
            middleware.len(),
            request.to
        );
        for mw in middleware {
            trace_log!(
                "Middleware '{}' after_navigation for '{}'",
                mw.name(),
                request.to
            );
            mw.after_navigation(cx, request);
        }
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
        let url = if let Some(url) = self.named_routes.url_for(name, params) {
            debug_log!("Named route '{}' resolved to '{}'", name, url);
            url
        } else {
            warn_log!("Named route '{}' not found in registry", name);
            return None;
        };
        Some(self.push(url, cx))
    }

    /// Generate a URL for a named route by substituting `params` into its pattern.
    ///
    /// Returns `None` if the name is not registered.
    #[must_use]
    pub fn url_for(&self, name: &str, params: &RouteParams) -> Option<String> {
        self.named_routes.url_for(name, params)
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Return the current navigation path (including any query string).
    #[must_use]
    pub fn current_path(&self) -> &str {
        self.state.current_path()
    }

    /// Query parameters of the current location.
    #[must_use]
    pub fn current_query(&self) -> QueryParams {
        QueryParams::from_location(self.state.current_path())
    }

    /// Get current route match (with caching, requires mutable).
    pub fn current_match(&mut self) -> Option<crate::RouteMatch> {
        self.state.current_match()
    }

    /// Get current route match (immutable, no caching).
    #[must_use]
    pub fn current_match_immutable(&self) -> Option<crate::RouteMatch> {
        self.state.current_match_immutable()
    }

    /// Get the current matched Route.
    #[must_use]
    pub fn current_route(&self) -> Option<&Arc<crate::route::Route>> {
        self.state.current_route()
    }

    /// Check if can go back.
    #[must_use]
    pub const fn can_go_back(&self) -> bool {
        self.state.can_go_back()
    }

    /// Check if can go forward.
    #[must_use]
    pub const fn can_go_forward(&self) -> bool {
        self.state.can_go_forward()
    }

    /// Get mutable state reference.
    pub const fn state_mut(&mut self) -> &mut RouterState {
        &mut self.state
    }

    /// Get state reference.
    #[must_use]
    pub const fn state(&self) -> &RouterState {
        &self.state
    }

    /// Get nested route cache (mutable).
    #[cfg(feature = "cache")]
    pub const fn nested_cache_mut(&mut self) -> &mut RouteCache {
        &mut self.nested_cache
    }

    /// Get nested route cache statistics.
    #[cfg(feature = "cache")]
    #[must_use]
    pub const fn cache_stats(&self) -> &CacheStats {
        self.nested_cache.stats()
    }

    // ========================================================================
    // Error handlers
    // ========================================================================

    /// Set custom error handlers for 404 and navigation errors.
    pub fn set_error_handlers(&mut self, handlers: ErrorHandlers) {
        self.error_handlers = handlers;
    }

    /// Get a reference to the current error handlers.
    #[must_use]
    pub const fn error_handlers(&self) -> &ErrorHandlers {
        &self.error_handlers
    }

    // ========================================================================
    // Component cache
    // ========================================================================

    /// Get a cached component view by key, marking it as recently used.
    #[must_use]
    pub fn get_cached_component(&self, key: &str) -> Option<&AnyView> {
        let cached = self.component_cache.get(key)?;
        cached.last_used.set(self.tick());
        Some(&cached.view)
    }

    /// Store a component view in the cache, evicting the least recently used
    /// entry if the cache is full.
    pub fn cache_component(&mut self, key: String, view: AnyView) {
        if !self.component_cache.contains_key(&key) {
            while self.component_cache.len() >= self.component_cache_capacity.max(1) {
                let Some(oldest) = self
                    .component_cache
                    .iter()
                    .min_by_key(|(_, cached)| cached.last_used.get())
                    .map(|(key, _)| key.clone())
                else {
                    break;
                };
                trace_log!("Evicting cached component '{}'", oldest);
                self.component_cache.remove(&oldest);
            }
        }
        let last_used = Cell::new(self.tick());
        self.component_cache
            .insert(key, CachedComponent { view, last_used });
    }

    /// Set how many component views are kept alive (default
    /// [`DEFAULT_COMPONENT_CACHE_CAPACITY`]). Shrinking evicts the least
    /// recently used views right away.
    pub fn set_component_cache_capacity(&mut self, capacity: usize) {
        self.component_cache_capacity = capacity.max(1);
        while self.component_cache.len() > self.component_cache_capacity {
            let Some(oldest) = self
                .component_cache
                .iter()
                .min_by_key(|(_, cached)| cached.last_used.get())
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.component_cache.remove(&oldest);
        }
    }

    /// Number of cached component views.
    #[must_use]
    pub fn component_cache_len(&self) -> usize {
        self.component_cache.len()
    }

    /// Drop the cached component views of the route registered at `route_path`
    /// (for example `"/users/:id"` or a child path like `"settings"`), so the
    /// next visit builds a fresh page. Returns how many views were dropped.
    pub fn evict_components(&mut self, route_path: &str) -> usize {
        let prefix = component_key_prefix(route_path);
        let before = self.component_cache.len();
        self.component_cache.retain(|key, _| !key.starts_with(&prefix));
        before - self.component_cache.len()
    }

    /// Drop every cached component view.
    pub fn clear_component_cache(&mut self) {
        self.component_cache.clear();
    }

    fn tick(&self) -> u64 {
        let next = self.cache_clock.get() + 1;
        self.cache_clock.set(next);
        next
    }

    /// The cached page view rendered for a match-stack entry, if any.
    fn page_view(&self, entry: &crate::resolve::MatchEntry) -> Option<&AnyView> {
        let prefix = component_key_prefix(&entry.route.config.path);
        let mut pairs: Vec<_> = entry.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
        pairs.sort();
        let with_params = format!("?{}", pairs.join("&"));
        self.component_cache.iter().find_map(|(key, cached)| {
            let rest = key.strip_prefix(&prefix)?;
            // `route:{path}:{TypeId}` or `route:{path}:{TypeId}?{params}`
            match rest.find('?') {
                None => Some(&cached.view),
                Some(pos) => (rest[pos..] == with_params).then_some(&cached.view),
            }
        })
    }

    /// Route paths of the levels being left that opted out of caching with
    /// [`Route::keep_alive(false)`](crate::Route::keep_alive).
    fn departing_non_keep_alive(&self, common: usize) -> Vec<String> {
        self.match_stack.entries()[common..]
            .iter()
            .filter(|entry| !entry.route.is_keep_alive())
            .map(|entry| entry.route.config.path.clone())
            .collect()
    }

    // ========================================================================
    // Route change events
    // ========================================================================

    /// The last completed navigation, if any.
    #[must_use]
    pub const fn last_change(&self) -> Option<&crate::RouteChangeEvent> {
        self.last_change.as_ref()
    }

    /// Counter incremented on every completed navigation.
    #[must_use]
    pub const fn change_count(&self) -> u64 {
        self.change_seq
    }

    // ========================================================================
    // Transitions
    // ========================================================================

    /// Set transition for the next navigation.
    #[cfg(feature = "transition")]
    pub const fn set_next_transition(&mut self, transition: Transition) {
        self.next_transition = Some(transition);
    }

    /// Get and consume the next transition override.
    #[cfg(feature = "transition")]
    pub const fn take_next_transition(&mut self) -> Option<Transition> {
        self.next_transition.take()
    }

    /// Check if there's a transition override set.
    #[cfg(feature = "transition")]
    #[must_use]
    pub const fn has_next_transition(&self) -> bool {
        self.next_transition.is_some()
    }

    /// Clear transition override.
    #[cfg(feature = "transition")]
    pub const fn clear_next_transition(&mut self) {
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
            component_cache_capacity: DEFAULT_COMPONENT_CACHE_CAPACITY,
            cache_clock: Cell::new(0),
            error_handlers: ErrorHandlers::new(),
            pending: None,
            nav_seq: 0,
            change_seq: 0,
            last_change: None,
            #[cfg(feature = "transition")]
            reverse_transitions_on_back: true,
        }
    }
}

impl Global for GlobalRouter {}

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

/// A navigation request as it moves through the pipeline.
#[derive(Debug, Clone)]
struct NavIntent {
    path: String,
    op: NavigateOp,
    state: Option<HistoryState>,
    /// Set when resuming after async guards, so they are not run again.
    resumed: bool,
}

impl NavIntent {
    const fn new(path: String, op: NavigateOp) -> Self {
        Self {
            path,
            op,
            state: None,
            resumed: false,
        }
    }

    /// The intent to follow when a guard or hook redirects to `to`.
    ///
    /// A replace stays a replace (so the redirect doesn't add a history entry);
    /// push, back and forward become a push of the redirect target.
    fn redirect(&self, to: String) -> Self {
        let op = match self.op {
            NavigateOp::Replace => NavigateOp::Replace,
            NavigateOp::Push | NavigateOp::Back | NavigateOp::Forward => NavigateOp::Push,
        };
        Self::new(to, op)
    }
}

/// A navigation waiting on async guards.
#[derive(Clone)]
struct PendingNavigation {
    id: u64,
    intent: NavIntent,
    redirect_depth: usize,
    future: Rc<RefCell<Option<crate::guards::GuardFuture>>>,
}

/// State restored when an `on_enter` hook denies a navigation.
struct RouterSnapshot {
    state: RouterState,
    match_stack: MatchStack,
    #[cfg(feature = "transition")]
    previous_stack: Option<MatchStack>,
}

/// A cached component view and when it was last rendered (for LRU eviction).
#[derive(Clone)]
struct CachedComponent {
    view: AnyView,
    last_used: Cell<u64>,
}

/// Number of leading levels two match stacks share (same route, same params).
fn common_prefix_len(current: &MatchStack, target: &MatchStack) -> usize {
    current
        .entries()
        .iter()
        .zip(target.entries())
        .take_while(|(a, b)| Arc::ptr_eq(&a.route, &b.route) && a.params == b.params)
        .count()
}

/// Cache-key prefix of every component view created for `route_path`.
fn component_key_prefix(route_path: &str) -> String {
    format!("route:{route_path}:")
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
        F: FnOnce(&mut GlobalRouter, &mut Self) -> R,
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
    let _ = run_navigation(cx, |router, cx| router.push(path, cx));
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
#[must_use]
pub struct NavigatorHandle<'a, C: BorrowAppContext> {
    cx: &'a mut C,
}

impl<C: BorrowAppContext + BorrowMut<App>> NavigatorHandle<'_, C> {
    /// Navigate to a new path.
    pub fn push(self, route: impl IntoRoute) -> Self {
        Navigator::push(self.cx, route);
        self
    }

    /// Replace current path without adding to history.
    pub fn replace(self, route: impl IntoRoute) -> Self {
        Navigator::replace(self.cx, route);
        self
    }

    /// Go back to the previous route.
    pub fn pop(self) -> Self {
        Navigator::back(self.cx);
        self
    }

    /// Go forward in history.
    pub fn forward(self) -> Self {
        Navigator::forward(self.cx);
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
    pub const fn of<C: BorrowAppContext + BorrowMut<App>>(cx: &mut C) -> NavigatorHandle<'_, C> {
        NavigatorHandle { cx }
    }

    /// Navigate to a new path.
    ///
    /// Use [`try_push`](Self::try_push) to learn whether a guard blocked it.
    pub fn push(cx: &mut (impl BorrowAppContext + BorrowMut<App>), route: impl IntoRoute) {
        let _ = Self::try_push(cx, route);
    }

    /// Navigate to a new path and return the pipeline's [`NavigationResult`].
    ///
    /// The result is [`NavigationResult::Pending`] while async guards run;
    /// the navigation then completes (or is blocked) on its own.
    pub fn try_push(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        route: impl IntoRoute,
    ) -> NavigationResult {
        let descriptor = route.into_route();
        debug_log!("Navigator::push: pushing path '{}'", descriptor.path);
        run_navigation(cx.borrow_mut(), |router, app| {
            router.push(descriptor.path, app)
        })
    }

    /// Replace current path without adding to history.
    pub fn replace(cx: &mut (impl BorrowAppContext + BorrowMut<App>), route: impl IntoRoute) {
        let _ = Self::try_replace(cx, route);
    }

    /// Replace the current path and return the pipeline's [`NavigationResult`].
    pub fn try_replace(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        route: impl IntoRoute,
    ) -> NavigationResult {
        let descriptor = route.into_route();
        run_navigation(cx.borrow_mut(), |router, app| {
            router.replace(descriptor.path, app)
        })
    }

    /// Push a new path with associated [`HistoryState`] data.
    pub fn push_with_state(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        route: impl IntoRoute,
        state: HistoryState,
    ) {
        let descriptor = route.into_route();
        let _ = run_navigation(cx.borrow_mut(), |router, app| {
            router.push_with_state(descriptor.path, state, app)
        });
    }

    /// Replace current path with associated [`HistoryState`] data.
    pub fn replace_with_state(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        route: impl IntoRoute,
        state: HistoryState,
    ) {
        let descriptor = route.into_route();
        let _ = run_navigation(cx.borrow_mut(), |router, app| {
            router.replace_with_state(descriptor.path, state, app)
        });
    }

    /// Return the current [`HistoryEntry`] (path + optional state).
    pub fn current_entry(cx: &App) -> HistoryEntry {
        cx.global::<GlobalRouter>().current_entry().clone()
    }

    /// Go back to the previous route.
    pub fn pop(cx: &mut (impl BorrowAppContext + BorrowMut<App>)) {
        let _ = Self::try_back(cx);
    }

    /// Alias for [`pop`](Navigator::pop).
    pub fn back(cx: &mut (impl BorrowAppContext + BorrowMut<App>)) {
        Self::pop(cx);
    }

    /// Go back and return the pipeline's result, or `None` at the start of history.
    pub fn try_back(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
    ) -> Option<NavigationResult> {
        run_optional_navigation(cx.borrow_mut(), GlobalRouter::back)
    }

    /// Go forward in history.
    pub fn forward(cx: &mut (impl BorrowAppContext + BorrowMut<App>)) {
        let _ = Self::try_forward(cx);
    }

    /// Go forward and return the pipeline's result, or `None` at the end of history.
    pub fn try_forward(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
    ) -> Option<NavigationResult> {
        run_optional_navigation(cx.borrow_mut(), GlobalRouter::forward)
    }

    /// Get current path (including any query string).
    pub fn current_path(cx: &App) -> String {
        cx.global::<GlobalRouter>().current_path().to_string()
    }

    /// Query parameters of the current location (`/users?page=2` → `page=2`).
    pub fn query(cx: &App) -> QueryParams {
        cx.global::<GlobalRouter>().current_query()
    }

    /// Route parameters of the deepest matched route.
    pub fn params(cx: &App) -> RouteParams {
        cx.global::<GlobalRouter>().match_stack().params()
    }

    /// Return `true` while a navigation waits on async guards.
    pub fn is_pending(cx: &App) -> bool {
        cx.global::<GlobalRouter>().is_pending()
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
        let _ = Self::try_push_named(cx, name, params);
    }

    /// Navigate to a named route and return the pipeline's result, or `None`
    /// if the name is unknown or a parameter is missing.
    pub fn try_push_named(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        name: &str,
        params: &RouteParams,
    ) -> Option<NavigationResult> {
        run_optional_navigation(cx.borrow_mut(), |router, app| {
            router.push_named(name, params, app)
        })
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
        let _ = run_navigation(cx.borrow_mut(), |router, app| {
            router.push_with_transition(descriptor.path, transition, app)
        });
    }

    /// Replace with a specific transition.
    #[cfg(feature = "transition")]
    pub fn replace_with_transition(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        route: impl IntoRoute,
        transition: Transition,
    ) {
        let descriptor = route.into_route();
        let _ = run_navigation(cx.borrow_mut(), |router, app| {
            router.replace_with_transition(descriptor.path, transition, app)
        });
    }

    /// Push named route with a specific transition.
    #[cfg(feature = "transition")]
    pub fn push_named_with_transition(
        cx: &mut (impl BorrowAppContext + BorrowMut<App>),
        name: &str,
        params: &RouteParams,
        transition: Transition,
    ) {
        let _ = run_optional_navigation(cx.borrow_mut(), |router, app| {
            router.set_next_transition(transition);
            router.push_named(name, params, app)
        });
    }
}

// ============================================================================
// Navigation driver
// ============================================================================

/// Run a navigation on the global router, refresh windows, and drive any
/// async guards it is waiting on.
fn run_navigation(
    cx: &mut App,
    navigate: impl FnOnce(&mut GlobalRouter, &App) -> NavigationResult,
) -> NavigationResult {
    run_optional_navigation(cx, |router, app| Some(navigate(router, app))).unwrap_or_else(|| {
        NavigationResult::Error(crate::error::NavigationError::NavigationFailed {
            message: "navigation returned no result".into(),
        })
    })
}

fn run_optional_navigation(
    cx: &mut App,
    navigate: impl FnOnce(&mut GlobalRouter, &App) -> Option<NavigationResult>,
) -> Option<NavigationResult> {
    let result = cx.update_global::<GlobalRouter, _>(|router, cx| navigate(router, cx));
    if result.as_ref().is_some_and(NavigationResult::is_pending) {
        drive_pending_navigation(cx);
    }
    cx.refresh_windows();
    result
}

/// Spawn the pending navigation's async guards on the foreground executor and
/// finish the navigation when they resolve.
fn drive_pending_navigation(cx: &mut App) {
    let Some((id, future)) = cx.global::<GlobalRouter>().take_pending_future() else {
        return;
    };
    cx.spawn(async move |cx| {
        let action = future.await;
        let _ = cx.update(|cx| {
            let result = cx.update_global::<GlobalRouter, _>(|router, cx| {
                router.resume_pending(id, action, cx)
            });
            if result.as_ref().is_some_and(NavigationResult::is_pending) {
                drive_pending_navigation(cx);
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::needless_pass_by_ref_mut)]
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

        cx.update(Navigator::pop);
        assert_eq!(cx.read(Navigator::current_path), "/page1");
        assert!(cx.read(Navigator::can_pop));
        assert!(cx.read(Navigator::can_go_forward));

        cx.update(Navigator::forward);
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

        cx.update(Navigator::pop);
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

        cx.update(Navigator::pop);
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

        cx.update(Navigator::pop);
        assert_eq!(cx.read(Navigator::current_path), "/step2");

        cx.update(Navigator::pop);
        assert_eq!(cx.read(Navigator::current_path), "/step1");

        cx.update(Navigator::pop);
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
            let _ = Navigator::of(cx).push("/home");
        });
        assert_eq!(cx.read(Navigator::current_path), "/home");

        cx.update(|cx| {
            let _ = Navigator::of(cx).push("/profile").pop();
        });
        assert_eq!(cx.read(Navigator::current_path), "/home");

        cx.update(|cx| {
            let _ = Navigator::of(cx).replace("/profile");
        });
        assert_eq!(cx.read(Navigator::current_path), "/profile");

        assert!(cx.read(Navigator::can_pop));
        cx.update(|cx| {
            let _ = Navigator::of(cx).pop();
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
        drop(log);
    }

}
