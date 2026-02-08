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

    /// Get the first top-level Route that matches the current path.
    ///
    /// With MatchStack architecture, rendering uses `GlobalRouter::match_stack()`.
    /// This method is kept for compatibility — it returns the first registered
    /// route whose pattern matches the current path (exact or prefix).
    pub fn current_route(&self) -> Option<&Arc<Route>> {
        let path = self.current_path();
        for route in &self.routes {
            if route.matches(path).is_some() {
                return Some(route);
            }
            // Also check if path is under this route (prefix match for nested routes)
            let route_path = route.config.path.trim_matches('/');
            let path_trimmed = path.trim_matches('/');
            if !route_path.is_empty()
                && path_trimmed.starts_with(route_path)
                && (path_trimmed.len() == route_path.len()
                    || path_trimmed[route_path.len()..].starts_with('/'))
            {
                return Some(route);
            }
            // Root route matches everything if it has children
            if route_path.is_empty() && !route.children.is_empty() {
                return Some(route);
            }
        }
        None
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

    /// Peek at the path we would navigate to on `back()`, without actually navigating.
    pub fn peek_back_path(&self) -> Option<&str> {
        if self.current > 0 {
            Some(&self.history[self.current - 1])
        } else {
            None
        }
    }

    /// Peek at the path we would navigate to on `forward()`, without actually navigating.
    pub fn peek_forward_path(&self) -> Option<&str> {
        if self.current < self.history.len() - 1 {
            Some(&self.history[self.current + 1])
        } else {
            None
        }
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
