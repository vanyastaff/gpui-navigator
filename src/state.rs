//! Router state management

use crate::route::Route;
use crate::{NavigationDirection, RouteChangeEvent, RouteMatch, RouteParams};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Router state
#[derive(Debug)]
pub struct RouterState {
    /// Navigation history stack
    history: Vec<String>,
    /// Current position in history
    current: usize,
    /// Registered routes
    routes: Vec<Arc<Route>>,
    /// Route cache
    cache: HashMap<String, RouteMatch>,
    /// Current route parameters (for parameter inheritance in nested routing)
    current_params: RouteParams,
    /// Navigation ID counter for cancellation tracking (T009)
    /// Each navigation increments this, allowing detection of stale navigations
    navigation_id: Arc<AtomicUsize>,
}

impl RouterState {
    /// Create a new router state
    pub fn new() -> Self {
        Self {
            history: vec!["/".to_string()],
            current: 0,
            routes: Vec::new(),
            cache: HashMap::new(),
            current_params: RouteParams::new(),
            navigation_id: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get current navigation ID
    pub fn navigation_id(&self) -> usize {
        self.navigation_id.load(Ordering::SeqCst)
    }

    /// Start a new navigation and return the new navigation ID
    ///
    /// This increments the navigation counter, allowing previous navigations
    /// to detect they've been superseded and should be cancelled.
    pub fn start_navigation(&self) -> usize {
        self.navigation_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Check if a navigation is still current (not cancelled by newer navigation)
    pub fn is_navigation_current(&self, nav_id: usize) -> bool {
        self.navigation_id() == nav_id
    }

    /// Register a route
    pub fn add_route(&mut self, route: Route) {
        self.routes.push(Arc::new(route));
        self.cache.clear();
    }

    /// Get current path
    pub fn current_path(&self) -> &str {
        &self.history[self.current]
    }

    /// Get all registered routes
    pub fn routes(&self) -> &[Arc<Route>] {
        &self.routes
    }

    /// Get current route parameters
    pub fn current_params(&self) -> &RouteParams {
        &self.current_params
    }

    /// Update current route parameters
    pub fn set_current_params(&mut self, params: RouteParams) {
        self.current_params = params;
    }

    /// Get current route match (with caching)
    pub fn current_match(&mut self) -> Option<RouteMatch> {
        let path = self.current_path();

        // Check cache first
        if let Some(cached) = self.cache.get(path) {
            return Some(cached.clone());
        }

        // Find matching route
        for route in &self.routes {
            if let Some(route_match) = route.matches(path) {
                self.cache.insert(path.to_string(), route_match.clone());
                return Some(route_match);
            }
        }

        None
    }

    /// Get current route match without caching (immutable)
    ///
    /// Use this when you need to access the current route from a non-mutable context,
    /// such as in a GPUI Render implementation.
    pub fn current_match_immutable(&self) -> Option<RouteMatch> {
        let path = self.current_path();

        // Check cache first
        if let Some(cached) = self.cache.get(path) {
            return Some(cached.clone());
        }

        // Find matching route without caching
        for route in &self.routes {
            if let Some(route_match) = route.matches(path) {
                return Some(route_match);
            }
        }

        None
    }

    /// Get the matched Route for current path
    ///
    /// For nested routes, returns the parent route that should render.
    /// For leaf routes, returns the route itself.
    ///
    /// Searches through the route tree recursively starting from registered routes.
    pub fn current_route(&self) -> Option<&Arc<Route>> {
        let path = self.current_path();
        find_matching_route_in_tree(
            &self.routes,
            path.trim_start_matches('/').trim_end_matches('/'),
        )
    }

    /// Get the route to render at the TOP level (for router_view)
    ///
    /// Returns the first-level route that matches the current path.
    /// Unlike current_route(), this DOES NOT recurse into children.
    /// This is used by router_view() to render top-level layouts.
    pub fn current_route_for_rendering(&self) -> Option<&Arc<Route>> {
        let path = self.current_path();
        let path_normalized = path.trim_start_matches('/').trim_end_matches('/');

        for route in &self.routes {
            let route_path = route
                .config
                .path
                .trim_start_matches('/')
                .trim_end_matches('/');

            // Check if path matches this route or is under it
            let matches = if route_path.is_empty() {
                // Root route matches everything if it has children, otherwise only empty path
                path_normalized.is_empty() || !route.get_children().is_empty()
            } else {
                // Exact match or path starts with route path
                path_normalized == route_path
                    || path_normalized.starts_with(&format!("{}/", route_path))
            };

            if matches {
                return Some(route);
            }
        }

        None
    }

    /// Find the deepest parent route with children that matches the path
    ///
    /// **DEPRECATED**: Use find_matching_route_in_tree instead.
    ///
    /// Returns parent route if path matches the route (exact or deeper) AND route has children.
    /// For exact matches, this allows RouterOutlet to render index routes.
    #[deprecated(since = "0.1.4", note = "Use find_matching_route_in_tree instead")]
    fn find_parent_with_children(&self, path: &str) -> Option<&Arc<Route>> {
        let path_normalized = path.trim_start_matches('/').trim_end_matches('/');

        let mut best_match: Option<&Arc<Route>> = None;
        let mut best_depth = 0;

        for route in &self.routes {
            // Skip routes without children
            if route.children.is_empty() {
                continue;
            }

            let route_path = route
                .config
                .path
                .trim_start_matches('/')
                .trim_end_matches('/');

            // Check if current path matches this route (exact or deeper)
            let matches = if route_path.is_empty() {
                // Root route "/" matches everything
                true
            } else {
                // Exact match OR path goes deeper than route
                path_normalized == route_path
                    || (path_normalized.starts_with(route_path)
                        && path_normalized.len() > route_path.len()
                        && path_normalized[route_path.len()..].starts_with('/'))
            };

            if matches {
                let depth = route_path.split('/').filter(|s| !s.is_empty()).count();

                // Prefer deeper routes, but for same depth prefer exact matches
                let is_better =
                    depth > best_depth || (depth == best_depth && path_normalized == route_path);

                if is_better {
                    best_match = Some(route);
                    best_depth = depth;
                }
            }
        }

        best_match
    }

    /// Navigate to a new path
    pub fn push(&mut self, path: String) -> RouteChangeEvent {
        let from = Some(self.current_path().to_string());

        // Remove forward history when pushing
        self.history.truncate(self.current + 1);

        // Add new path
        self.history.push(path.clone());
        self.current += 1;

        RouteChangeEvent {
            from,
            to: path,
            direction: NavigationDirection::Forward,
        }
    }

    /// Replace current path
    pub fn replace(&mut self, path: String) -> RouteChangeEvent {
        let from = Some(self.current_path().to_string());

        self.history[self.current] = path.clone();

        RouteChangeEvent {
            from,
            to: path,
            direction: NavigationDirection::Replace,
        }
    }

    /// Go back in history
    pub fn back(&mut self) -> Option<RouteChangeEvent> {
        if self.current > 0 {
            let from = Some(self.current_path().to_string());
            self.current -= 1;
            let to = self.current_path().to_string();

            Some(RouteChangeEvent {
                from,
                to,
                direction: NavigationDirection::Back,
            })
        } else {
            None
        }
    }

    /// Go forward in history
    pub fn forward(&mut self) -> Option<RouteChangeEvent> {
        if self.current < self.history.len() - 1 {
            let from = Some(self.current_path().to_string());
            self.current += 1;
            let to = self.current_path().to_string();

            Some(RouteChangeEvent {
                from,
                to,
                direction: NavigationDirection::Forward,
            })
        } else {
            None
        }
    }

    /// Check if can go back
    pub fn can_go_back(&self) -> bool {
        self.current > 0
    }

    /// Check if can go forward
    pub fn can_go_forward(&self) -> bool {
        self.current < self.history.len() - 1
    }

    /// Clear navigation history
    pub fn clear(&mut self) {
        self.history.clear();
        self.history.push("/".to_string());
        self.current = 0;
        self.cache.clear();
    }
}

impl Default for RouterState {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for RouterState {
    fn clone(&self) -> Self {
        Self {
            history: self.history.clone(),
            current: self.current,
            routes: self.routes.clone(),
            cache: self.cache.clone(),
            current_params: self.current_params.clone(),
            // Clone Arc, not the AtomicUsize value - share navigation_id across clones
            navigation_id: Arc::clone(&self.navigation_id),
        }
    }
}

/// Router - manages navigation state
pub struct Router {
    state: RouterState,
}

impl Router {
    /// Create a new router
    pub fn new() -> Self {
        Self {
            state: RouterState::new(),
        }
    }

    /// Get mutable reference to state
    pub fn state_mut(&mut self) -> &mut RouterState {
        &mut self.state
    }

    /// Get reference to state
    pub fn state(&self) -> &RouterState {
        &self.state
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the route that should render for the given path
///
/// Recursively searches through the route tree to find the most appropriate route.
/// For paths with nested routes, returns the parent route that should render (not the final leaf).
/// For leaf routes without children, returns the route itself.
///
/// # Algorithm
///
/// 1. Try each top-level route
/// 2. For each route, check if it matches the path (exact or deeper)
/// 3. If it matches and has children, recursively search children
/// 4. Return the deepest matching parent route (with children), or leaf route (without children)
///
/// # Examples
///
/// ```text
/// Routes:
///   /dashboard (has children)
///     /overview
///     /analytics
///
/// For path "/dashboard/analytics":
///   Returns: /dashboard route (parent with children)
///   RouterOutlet in /dashboard will then render /analytics child
///
/// For path "/about" (no children):
///   Returns: /about route itself
/// ```
fn find_matching_route_in_tree<'a>(routes: &'a [Arc<Route>], path: &str) -> Option<&'a Arc<Route>> {
    let path_normalized = path.trim_start_matches('/').trim_end_matches('/');

    // Try each top-level route
    for route in routes {
        if let Some(matched) = find_matching_route(route, path_normalized, "") {
            return Some(matched);
        }
    }

    None
}

/// Recursively find matching route in the tree
///
/// accumulated_path - the path built up from parent routes
fn find_matching_route<'a>(
    route: &'a Arc<Route>,
    path: &str,
    accumulated_path: &str,
) -> Option<&'a Arc<Route>> {
    let route_path = route
        .config
        .path
        .trim_start_matches('/')
        .trim_end_matches('/');

    // Build full path for this route
    let full_route_path = if accumulated_path.is_empty() {
        if route_path.is_empty() {
            String::new()
        } else {
            route_path.to_string()
        }
    } else if route_path.is_empty() {
        accumulated_path.to_string()
    } else {
        format!("{}/{}", accumulated_path, route_path)
    };

    // Check if current path matches this route
    let matches = if full_route_path.is_empty() {
        // Root route - only matches empty path or if it has children (then it can match deeper paths)
        path.is_empty() || !route.children.is_empty()
    } else {
        // Check for exact match or path going deeper
        path == full_route_path
            || (path.starts_with(&full_route_path)
                && path.len() > full_route_path.len()
                && path[full_route_path.len()..].starts_with('/'))
    };

    if !matches {
        return None;
    }

    // If this route has children, try to find a deeper match
    if !route.children.is_empty() {
        // First, check if any child matches (recursively)
        for child in &route.children {
            if let Some(matched) = find_matching_route(child, path, &full_route_path) {
                return Some(matched);
            }
        }

        // No child matched but path is under this route
        // Return this route (RouterOutlet will render the appropriate child)
        if path == full_route_path || path.starts_with(&format!("{}/", full_route_path)) {
            return Some(route);
        }
    }

    // Exact match and no children - return this route
    if path == full_route_path {
        Some(route)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation() {
        let mut state = RouterState::new();

        assert_eq!(state.current_path(), "/");

        state.push("/users".to_string());
        assert_eq!(state.current_path(), "/users");

        state.push("/users/123".to_string());
        assert_eq!(state.current_path(), "/users/123");

        state.back();
        assert_eq!(state.current_path(), "/users");

        state.forward();
        assert_eq!(state.current_path(), "/users/123");
    }

    #[test]
    fn test_replace() {
        let mut state = RouterState::new();

        state.push("/users".to_string());
        state.replace("/posts".to_string());

        assert_eq!(state.current_path(), "/posts");
        assert_eq!(state.history.len(), 2);
    }
}
