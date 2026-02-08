//! Router context integration for GPUI (UPDATED for MatchStack)
//!
//! Key change: GlobalRouter now resolves a MatchStack on every navigation,
//! so outlets can read their route by depth index instead of searching.

#[cfg(feature = "cache")]
use crate::cache::{CacheStats, RouteCache};
use crate::resolve::{resolve_match_stack, MatchStack};
use crate::route::NamedRouteRegistry;
#[cfg(feature = "transition")]
use crate::transition::Transition;
use crate::{IntoRoute, Route, RouteChangeEvent, RouteParams, RouterState};
use gpui::{App, BorrowAppContext, Global};

// ============================================================================
// NavigationRequest (unchanged)
// ============================================================================

pub struct NavigationRequest {
    pub from: Option<String>,
    pub to: String,
    pub params: RouteParams,
}

impl NavigationRequest {
    pub fn new(to: String) -> Self {
        Self {
            from: None,
            to,
            params: RouteParams::new(),
        }
    }

    pub fn with_from(to: String, from: String) -> Self {
        Self {
            from: Some(from),
            to,
            params: RouteParams::new(),
        }
    }

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
// GlobalRouter (UPDATED)
// ============================================================================

#[derive(Clone)]
pub struct GlobalRouter {
    state: RouterState,

    /// ────────────────────── NEW ──────────────────────
    /// Pre-resolved route chain for the current path.
    /// Built once per navigation, consumed by outlets during render.
    match_stack: MatchStack,

    /// Previous match stack — used for transition exit animations.
    /// Contains the stack from before the last navigation.
    #[cfg(feature = "transition")]
    previous_stack: Option<MatchStack>,
    /// ─────────────────── END NEW ─────────────────────

    #[cfg(feature = "cache")]
    nested_cache: RouteCache,
    named_routes: NamedRouteRegistry,
    #[cfg(feature = "transition")]
    next_transition: Option<Transition>,
}

impl GlobalRouter {
    pub fn new() -> Self {
        Self {
            state: RouterState::new(),
            match_stack: MatchStack::new(), // ← NEW
            #[cfg(feature = "transition")]
            previous_stack: None, // ← NEW
            #[cfg(feature = "cache")]
            nested_cache: RouteCache::new(),
            named_routes: NamedRouteRegistry::new(),
            #[cfg(feature = "transition")]
            next_transition: None,
        }
    }

    // ══════════════════════════════════════════════════
    // NEW: MatchStack access
    // ══════════════════════════════════════════════════

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
    ///
    /// Called after add_route() or when routes are modified.
    fn re_resolve(&mut self) {
        self.match_stack = resolve_match_stack(
            self.state.routes(),
            self.state.current_path(),
        );
    }

    // ══════════════════════════════════════════════════
    // Updated navigation methods
    // ══════════════════════════════════════════════════

    pub fn add_route(&mut self, route: Route) {
        if let Some(name) = &route.config.name {
            self.named_routes
                .register(name.clone(), route.config.path.clone());
        }

        self.state.add_route(route);

        // Re-resolve after adding routes
        self.re_resolve();
    }

    pub fn push(&mut self, path: String) -> RouteChangeEvent {
        // Save previous stack for transitions
        #[cfg(feature = "transition")]
        {
            self.previous_stack = Some(self.match_stack.clone());
        }

        let event = self.state.push(path);

        // ── NEW: resolve match stack immediately ──
        self.match_stack = resolve_match_stack(
            self.state.routes(),
            self.state.current_path(),
        );

        event
    }

    pub fn replace(&mut self, path: String) -> RouteChangeEvent {
        #[cfg(feature = "transition")]
        {
            self.previous_stack = Some(self.match_stack.clone());
        }

        let event = self.state.replace(path);

        // ── NEW: resolve match stack immediately ──
        self.match_stack = resolve_match_stack(
            self.state.routes(),
            self.state.current_path(),
        );

        event
    }

    pub fn back(&mut self) -> Option<RouteChangeEvent> {
        #[cfg(feature = "transition")]
        {
            self.previous_stack = Some(self.match_stack.clone());
        }

        let event = self.state.back()?;

        // ── NEW: resolve match stack immediately ──
        self.match_stack = resolve_match_stack(
            self.state.routes(),
            self.state.current_path(),
        );

        Some(event)
    }

    pub fn forward(&mut self) -> Option<RouteChangeEvent> {
        #[cfg(feature = "transition")]
        {
            self.previous_stack = Some(self.match_stack.clone());
        }

        let event = self.state.forward()?;

        // ── NEW: resolve match stack immediately ──
        self.match_stack = resolve_match_stack(
            self.state.routes(),
            self.state.current_path(),
        );

        Some(event)
    }

    // ══════════════════════════════════════════════════
    // Unchanged methods
    // ══════════════════════════════════════════════════

    pub fn push_named(&mut self, name: &str, params: &RouteParams) -> Option<RouteChangeEvent> {
        let url = self.named_routes.url_for(name, params)?;
        Some(self.push(url))
    }

    pub fn url_for(&self, name: &str, params: &RouteParams) -> Option<String> {
        self.named_routes.url_for(name, params)
    }

    pub fn current_path(&self) -> &str {
        self.state.current_path()
    }

    pub fn current_match(&mut self) -> Option<crate::RouteMatch> {
        self.state.current_match()
    }

    pub fn current_match_immutable(&self) -> Option<crate::RouteMatch> {
        self.state.current_match_immutable()
    }

    pub fn current_route(&self) -> Option<&std::sync::Arc<crate::route::Route>> {
        self.state.current_route()
    }

    pub fn can_go_back(&self) -> bool {
        self.state.can_go_back()
    }

    pub fn can_go_forward(&self) -> bool {
        self.state.can_go_forward()
    }

    pub fn state_mut(&mut self) -> &mut RouterState {
        &mut self.state
    }

    pub fn state(&self) -> &RouterState {
        &self.state
    }

    #[cfg(feature = "cache")]
    pub fn nested_cache_mut(&mut self) -> &mut RouteCache {
        &mut self.nested_cache
    }

    #[cfg(feature = "cache")]
    pub fn cache_stats(&self) -> &CacheStats {
        self.nested_cache.stats()
    }

    #[cfg(feature = "transition")]
    pub fn set_next_transition(&mut self, transition: Transition) {
        self.next_transition = Some(transition);
    }

    #[cfg(feature = "transition")]
    pub fn take_next_transition(&mut self) -> Option<Transition> {
        self.next_transition.take()
    }

    #[cfg(feature = "transition")]
    pub fn has_next_transition(&self) -> bool {
        self.next_transition.is_some()
    }

    #[cfg(feature = "transition")]
    pub fn clear_next_transition(&mut self) {
        self.next_transition = None;
    }

    #[cfg(feature = "transition")]
    pub fn push_with_transition(
        &mut self,
        path: String,
        transition: Transition,
    ) -> RouteChangeEvent {
        self.set_next_transition(transition);
        self.push(path)
    }

    #[cfg(feature = "transition")]
    pub fn replace_with_transition(
        &mut self,
        path: String,
        transition: Transition,
    ) -> RouteChangeEvent {
        self.set_next_transition(transition);
        self.replace(path)
    }
}

impl Default for GlobalRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl Global for GlobalRouter {}

// ============================================================================
// Navigator (updated to use resolve)
// ============================================================================
// The rest of Navigator methods remain unchanged — they call GlobalRouter
// methods which now automatically resolve the match stack.
//
// Example:
//   Navigator::push(cx, "/dashboard/settings")
//     → cx.update_global::<GlobalRouter>(|router, _| router.push(path))
//       → state.push(path)
//       → resolve_match_stack(routes, path)  ← automatic!
//
// No changes needed to Navigator API.
