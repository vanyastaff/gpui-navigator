//! Router context integration for GPUI
//!
//! This module provides the global router state management through GPUI's context system.
//! It exposes the `Navigator` API for navigation operations and manages router lifecycle.

#[cfg(feature = "cache")]
use crate::cache::{CacheStats, RouteCache};
use crate::route::NamedRouteRegistry;
#[cfg(feature = "transition")]
use crate::transition::Transition;
use crate::{IntoRoute, Route, RouteChangeEvent, RouteParams, RouterState};
use gpui::{App, BorrowAppContext, Global};

// ============================================================================
// NavigationRequest
// ============================================================================

/// Request for navigation.
///
/// Contains information about the navigation being performed.
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
    /// Create a new navigation request
    pub fn new(to: String) -> Self {
        Self {
            from: None,
            to,
            params: RouteParams::new(),
        }
    }

    /// Create a navigation request with a source path
    pub fn with_from(to: String, from: String) -> Self {
        Self {
            from: Some(from),
            to,
            params: RouteParams::new(),
        }
    }

    /// Set route parameters
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

/// Global router state accessible from any component
#[derive(Clone)]
pub struct GlobalRouter {
    state: RouterState,
    /// Cache for nested route resolution
    #[cfg(feature = "cache")]
    nested_cache: RouteCache,
    /// Registry for named routes
    named_routes: NamedRouteRegistry,
    /// Transition override for next navigation
    #[cfg(feature = "transition")]
    next_transition: Option<Transition>,
}

impl GlobalRouter {
    /// Create a new global router
    pub fn new() -> Self {
        Self {
            state: RouterState::new(),
            #[cfg(feature = "cache")]
            nested_cache: RouteCache::new(),
            named_routes: NamedRouteRegistry::new(),
            #[cfg(feature = "transition")]
            next_transition: None,
        }
    }

    /// Register a route
    pub fn add_route(&mut self, route: Route) {
        // Register named route if it has a name
        if let Some(name) = &route.config.name {
            self.named_routes
                .register(name.clone(), route.config.path.clone());
        }

        self.state.add_route(route);
        // Clear cache when routes change
        #[cfg(feature = "cache")]
        self.nested_cache.clear();
    }

    /// Navigate to a named route with parameters
    pub fn push_named(&mut self, name: &str, params: &RouteParams) -> Option<RouteChangeEvent> {
        let url = self.named_routes.url_for(name, params)?;
        Some(self.push(url))
    }

    /// Generate URL for a named route
    pub fn url_for(&self, name: &str, params: &RouteParams) -> Option<String> {
        self.named_routes.url_for(name, params)
    }

    /// Navigate to a path
    pub fn push(&mut self, path: String) -> RouteChangeEvent {
        // Clear cache on navigation
        #[cfg(feature = "cache")]
        self.nested_cache.clear();
        self.state.push(path)
    }

    /// Replace current path
    pub fn replace(&mut self, path: String) -> RouteChangeEvent {
        // Clear cache on navigation
        #[cfg(feature = "cache")]
        self.nested_cache.clear();
        self.state.replace(path)
    }

    /// Go back
    pub fn back(&mut self) -> Option<RouteChangeEvent> {
        // Clear cache on navigation
        #[cfg(feature = "cache")]
        self.nested_cache.clear();
        self.state.back()
    }

    /// Go forward
    pub fn forward(&mut self) -> Option<RouteChangeEvent> {
        // Clear cache on navigation
        #[cfg(feature = "cache")]
        self.nested_cache.clear();
        self.state.forward()
    }

    /// Get current path
    pub fn current_path(&self) -> &str {
        self.state.current_path()
    }

    /// Get current route match (with caching, requires mutable)
    pub fn current_match(&mut self) -> Option<crate::RouteMatch> {
        self.state.current_match()
    }

    /// Get current route match (immutable, no caching)
    ///
    /// Use this from Render implementations and other immutable contexts
    pub fn current_match_immutable(&self) -> Option<crate::RouteMatch> {
        self.state.current_match_immutable()
    }

    /// Get the current matched Route
    ///
    /// Returns the shared `Arc<Route>` that matched the current path.
    /// Useful for accessing the route's children and builder without cloning.
    pub fn current_route(&self) -> Option<&std::sync::Arc<crate::route::Route>> {
        self.state.current_route()
    }

    /// Check if can go back
    pub fn can_go_back(&self) -> bool {
        self.state.can_go_back()
    }

    /// Check if can go forward
    pub fn can_go_forward(&self) -> bool {
        self.state.can_go_forward()
    }

    /// Get mutable state reference
    pub fn state_mut(&mut self) -> &mut RouterState {
        &mut self.state
    }

    /// Get state reference
    pub fn state(&self) -> &RouterState {
        &self.state
    }

    /// Get nested route cache (mutable)
    #[cfg(feature = "cache")]
    pub fn nested_cache_mut(&mut self) -> &mut RouteCache {
        &mut self.nested_cache
    }

    /// Get nested route cache statistics
    #[cfg(feature = "cache")]
    pub fn cache_stats(&self) -> &CacheStats {
        self.nested_cache.stats()
    }

    /// Set transition for the next navigation
    ///
    /// This override will be used for the next push/replace operation,
    /// then automatically cleared.
    ///
    /// # Example
    /// ```ignore
    /// use gpui_navigator::{GlobalRouter, Transition};
    ///
    /// cx.update_global::<GlobalRouter, _>(|router, _| {
    ///     router.set_next_transition(Transition::fade(300));
    ///     router.push("/page".to_string());
    /// });
    /// ```
    #[cfg(feature = "transition")]
    pub fn set_next_transition(&mut self, transition: Transition) {
        self.next_transition = Some(transition);
    }

    /// Get and consume the next transition override
    ///
    /// Returns the transition override if set, and clears it.
    /// Used internally by navigation methods.
    #[cfg(feature = "transition")]
    pub fn take_next_transition(&mut self) -> Option<Transition> {
        self.next_transition.take()
    }

    /// Check if there's a transition override set
    #[cfg(feature = "transition")]
    pub fn has_next_transition(&self) -> bool {
        self.next_transition.is_some()
    }

    /// Clear transition override without consuming it
    #[cfg(feature = "transition")]
    pub fn clear_next_transition(&mut self) {
        self.next_transition = None;
    }

    /// Navigate with a specific transition
    ///
    /// Convenience method that sets the transition and navigates in one call.
    ///
    /// # Example
    /// ```ignore
    /// use gpui_navigator::{GlobalRouter, Transition};
    ///
    /// cx.update_global::<GlobalRouter, _>(|router, _| {
    ///     router.push_with_transition("/page".to_string(), Transition::slide_left(300));
    /// });
    /// ```
    #[cfg(feature = "transition")]
    pub fn push_with_transition(
        &mut self,
        path: String,
        transition: Transition,
    ) -> RouteChangeEvent {
        self.set_next_transition(transition);
        self.push(path)
    }

    /// Replace with a specific transition
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

/// Trait for accessing the global router from context
pub trait UseRouter {
    /// Get reference to global router
    fn router(&self) -> &GlobalRouter;

    /// Update global router
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

/// Initialize global router with routes
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::{init_router, Route};
///
/// fn main() {
///     App::new().run(|cx| {
///         init_router(cx, |router| {
///             router.add_route(Route::new("/", |_, _cx, _params| gpui::div().into_any_element()));
///             router.add_route(Route::new("/users/:id", |_, _cx, _params| gpui::div().into_any_element()));
///         });
///     });
/// }
/// ```
pub fn init_router<F>(cx: &mut App, configure: F)
where
    F: FnOnce(&mut GlobalRouter),
{
    let mut router = GlobalRouter::new();
    configure(&mut router);
    cx.set_global(router);
}

/// Navigate to a path using global router
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::navigate;
///
/// // In any component with access to App
/// navigate(cx, "/users/123");
/// ```
pub fn navigate(cx: &mut App, path: impl Into<String>) {
    cx.update_router(|router, _cx| {
        router.push(path.into());
    });
}

/// Get current path from global router
pub fn current_path(cx: &App) -> String {
    cx.router().current_path().to_string()
}

/// Handle for Navigator.of(context) pattern
///
/// Provides instance methods for chained navigation calls.
pub struct NavigatorHandle<'a, C: BorrowAppContext> {
    cx: &'a mut C,
}

impl<C: BorrowAppContext> NavigatorHandle<'_, C> {
    /// Navigate to a new path
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::{Navigator, PageRoute};
    ///
    /// // Simple path
    /// Navigator::of(cx).push("/users");
    ///
    /// // With PageRoute
    /// Navigator::of(cx).push(PageRoute::builder("/users/:id", |_, _cx, _params| gpui::div())
    ///     .with_param("id".into(), "123".into()));
    /// ```
    pub fn push(self, route: impl IntoRoute) -> Self {
        let descriptor = route.into_route();
        self.cx.update_global::<GlobalRouter, _>(|router, _| {
            router.push(descriptor.path);
        });
        self
    }

    /// Replace current path without adding to history
    pub fn replace(self, route: impl IntoRoute) -> Self {
        let descriptor = route.into_route();
        self.cx.update_global::<GlobalRouter, _>(|router, _| {
            router.replace(descriptor.path);
        });
        self
    }

    /// Go back to the previous route
    pub fn pop(self) -> Self {
        self.cx.update_global::<GlobalRouter, _>(|router, _| {
            router.back();
        });
        self
    }

    /// Go forward in history
    pub fn forward(self) -> Self {
        self.cx.update_global::<GlobalRouter, _>(|router, _| {
            router.forward();
        });
        self
    }
}

/// Navigation API for convenient route navigation
///
/// Provides static methods for navigation operations:
/// - `Navigator::push(cx, "/path")` - Navigate to a new page
/// - `Navigator::pop(cx)` - Go back to previous page
/// - `Navigator::replace(cx, "/path")` - Replace current page
///
/// Works with any context that has access to App (`Context<V>`, `App`, etc.)
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::Navigator;
///
/// // Navigate to a new route
/// Navigator::push(cx, "/users/123");
///
/// // Go back
/// Navigator::pop(cx);
///
/// // Replace current route
/// Navigator::replace(cx, "/login");
/// ```
pub struct Navigator;

impl Navigator {
    /// Get a NavigatorHandle for the given context
    ///
    /// This allows chained navigation calls:
    /// ```ignore
    /// use gpui_navigator::Navigator;
    ///
    /// // Chained style
    /// Navigator::of(cx).push("/users");
    /// Navigator::of(cx).pop();
    ///
    /// // Or direct style (also works)
    /// Navigator::push(cx, "/users");
    /// Navigator::pop(cx);
    /// ```
    pub fn of<C: BorrowAppContext>(cx: &mut C) -> NavigatorHandle<'_, C> {
        NavigatorHandle { cx }
    }

    /// Navigate to a new path
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::{Navigator, PageRoute};
    ///
    /// // Simple string path
    /// Navigator::push(cx, "/users/123");
    ///
    /// // With PageRoute and params
    /// Navigator::push(cx, PageRoute::builder("/profile", |_, _cx, _params| gpui::div())
    ///     .with_param("userId".into(), "456".into()));
    /// ```
    pub fn push(cx: &mut impl BorrowAppContext, route: impl IntoRoute) {
        let descriptor = route.into_route();
        crate::debug_log!("Navigator::push: pushing path '{}'", descriptor.path);
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.push(descriptor.path);
        });
    }

    /// Replace current path without adding to history
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::{Navigator, PageRoute};
    ///
    /// // Simple string path
    /// Navigator::replace(cx, "/login");
    ///
    /// // With PageRoute
    /// Navigator::replace(cx, PageRoute::builder("/login", |_, _cx, _params| gpui::div())
    ///     .with_param("redirect".into(), "/dashboard".into()));
    /// ```
    pub fn replace(cx: &mut impl BorrowAppContext, route: impl IntoRoute) {
        let descriptor = route.into_route();
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.replace(descriptor.path);
        });
    }

    /// Go back to the previous route
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::Navigator;
    ///
    /// if Navigator::can_pop(cx) {
    ///     Navigator::pop(cx);
    /// }
    /// ```
    pub fn pop(cx: &mut impl BorrowAppContext) {
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.back();
        });
    }

    /// Alias for pop() - go back (kept for compatibility)
    pub fn back(cx: &mut impl BorrowAppContext) {
        Self::pop(cx);
    }

    /// Go forward in history
    pub fn forward(cx: &mut impl BorrowAppContext) {
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.forward();
        });
    }

    /// Get current path
    ///
    /// Works with `Context<V>` since it derefs to App
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::Navigator;
    ///
    /// let path = Navigator::current_path(cx);
    /// ```
    pub fn current_path(cx: &App) -> String {
        cx.global::<GlobalRouter>().current_path().to_string()
    }

    /// Check if can go back
    pub fn can_pop(cx: &App) -> bool {
        cx.global::<GlobalRouter>().can_go_back()
    }

    /// Alias for can_pop() - check if can go back (kept for compatibility)
    pub fn can_go_back(cx: &App) -> bool {
        Self::can_pop(cx)
    }

    /// Navigate to a named route with parameters
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::{Navigator, RouteParams};
    ///
    /// let mut params = RouteParams::new();
    /// params.set("id".into(), "123".into());
    ///
    /// Navigator::push_named(cx, "user.detail", &params);
    /// ```
    pub fn push_named(cx: &mut impl BorrowAppContext, name: &str, params: &RouteParams) {
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.push_named(name, params);
        });
    }

    /// Generate URL for a named route
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::{Navigator, RouteParams};
    ///
    /// let mut params = RouteParams::new();
    /// params.set("id".into(), "123".into());
    ///
    /// let url = Navigator::url_for(cx, "user.detail", &params);
    /// assert_eq!(url, Some("/users/123".to_string()));
    /// ```
    pub fn url_for(cx: &App, name: &str, params: &RouteParams) -> Option<String> {
        cx.global::<GlobalRouter>().url_for(name, params)
    }

    /// Check if can go forward
    pub fn can_go_forward(cx: &App) -> bool {
        cx.global::<GlobalRouter>().can_go_forward()
    }

    /// Set transition for the next navigation
    ///
    /// The transition will be used for the next push/replace call,
    /// then automatically cleared.
    ///
    /// # Example
    /// ```ignore
    /// use gpui_navigator::{Navigator, Transition};
    ///
    /// Navigator::set_next_transition(cx, Transition::fade(300));
    /// Navigator::push(cx, "/page");
    /// ```
    #[cfg(feature = "transition")]
    pub fn set_next_transition(cx: &mut impl BorrowAppContext, transition: Transition) {
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.set_next_transition(transition);
        });
    }

    /// Navigate with a specific transition
    ///
    /// # Example
    /// ```ignore
    /// use gpui_navigator::{Navigator, Transition};
    ///
    /// Navigator::push_with_transition(cx, "/page", Transition::slide_left(300));
    /// ```
    #[cfg(feature = "transition")]
    pub fn push_with_transition(
        cx: &mut impl BorrowAppContext,
        route: impl IntoRoute,
        transition: Transition,
    ) {
        let descriptor = route.into_route();
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.push_with_transition(descriptor.path, transition);
        });
    }

    /// Replace with a specific transition
    ///
    /// # Example
    /// ```ignore
    /// use gpui_navigator::{Navigator, Transition};
    ///
    /// Navigator::replace_with_transition(cx, "/page", Transition::fade(200));
    /// ```
    #[cfg(feature = "transition")]
    pub fn replace_with_transition(
        cx: &mut impl BorrowAppContext,
        route: impl IntoRoute,
        transition: Transition,
    ) {
        let descriptor = route.into_route();
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.replace_with_transition(descriptor.path, transition);
        });
    }

    /// Push named route with a specific transition
    ///
    /// # Example
    /// ```ignore
    /// use gpui_navigator::{Navigator, RouteParams, Transition};
    ///
    /// let mut params = RouteParams::new();
    /// params.set("id".to_string(), "123".to_string());
    /// Navigator::push_named_with_transition(
    ///     cx,
    ///     "user.detail",
    ///     &params,
    ///     Transition::slide_right(300)
    /// );
    /// ```
    #[cfg(feature = "transition")]
    pub fn push_named_with_transition(
        cx: &mut impl BorrowAppContext,
        name: &str,
        params: &RouteParams,
        transition: Transition,
    ) {
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.set_next_transition(transition);
            router.push_named(name, params);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{IntoElement, TestAppContext};

    #[gpui::test]
    fn test_nav_push(cx: &mut TestAppContext) {
        // Initialize router
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

        // Test initial state
        let initial_path = cx.read(Navigator::current_path);
        assert_eq!(initial_path, "/");

        // Test push navigation
        cx.update(|cx| {
            Navigator::push(cx, "/users");
        });

        let current_path = cx.read(Navigator::current_path);
        assert_eq!(current_path, "/users");

        // Test push with parameters
        cx.update(|cx| {
            Navigator::push(cx, "/users/123");
        });

        let current_path = cx.read(Navigator::current_path);
        assert_eq!(current_path, "/users/123");
    }

    #[gpui::test]
    fn test_nav_back_forward(cx: &mut TestAppContext) {
        // Initialize router
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

        // Navigate to multiple pages
        cx.update(|cx| {
            Navigator::push(cx, "/page1");
            Navigator::push(cx, "/page2");
        });

        assert_eq!(cx.read(Navigator::current_path), "/page2");
        assert!(cx.read(Navigator::can_pop));

        // Test back navigation
        cx.update(|cx| {
            Navigator::pop(cx);
        });

        assert_eq!(cx.read(Navigator::current_path), "/page1");
        assert!(cx.read(Navigator::can_pop));
        assert!(cx.read(Navigator::can_go_forward));

        // Test forward navigation
        cx.update(|cx| {
            Navigator::forward(cx);
        });

        assert_eq!(cx.read(Navigator::current_path), "/page2");
        assert!(!cx.read(Navigator::can_go_forward));
    }

    #[gpui::test]
    fn test_nav_replace(cx: &mut TestAppContext) {
        // Initialize router
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

        // Navigate and then replace
        cx.update(|cx| {
            Navigator::push(cx, "/login");
            Navigator::replace(cx, "/home");
        });

        assert_eq!(cx.read(Navigator::current_path), "/home");

        // After replace, going back should skip the replaced route
        cx.update(|cx| {
            Navigator::pop(cx);
        });

        assert_eq!(cx.read(Navigator::current_path), "/");
    }

    #[gpui::test]
    fn test_nav_can_go_back_boundaries(cx: &mut TestAppContext) {
        // Initialize router
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        // At initial state, can't go back
        assert!(!cx.read(Navigator::can_pop));

        // After navigation, can go back
        cx.update(|cx| {
            Navigator::push(cx, "/page1");
        });

        assert!(cx.read(Navigator::can_pop));

        // After going back, can't go back further
        cx.update(|cx| {
            Navigator::pop(cx);
        });

        assert!(!cx.read(Navigator::can_pop));
    }

    #[gpui::test]
    fn test_nav_multiple_pushes(cx: &mut TestAppContext) {
        // Initialize router
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

        // Navigate through multiple pages
        cx.update(|cx| {
            Navigator::push(cx, "/step1");
            Navigator::push(cx, "/step2");
            Navigator::push(cx, "/step3");
        });

        assert_eq!(cx.read(Navigator::current_path), "/step3");

        // Go back multiple times
        cx.update(|cx| {
            Navigator::pop(cx);
        });
        assert_eq!(cx.read(Navigator::current_path), "/step2");

        cx.update(|cx| {
            Navigator::pop(cx);
        });
        assert_eq!(cx.read(Navigator::current_path), "/step1");

        cx.update(|cx| {
            Navigator::pop(cx);
        });
        assert_eq!(cx.read(Navigator::current_path), "/");
    }

    #[gpui::test]
    fn test_nav_with_route_parameters(cx: &mut TestAppContext) {
        // Initialize router
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

        // Navigate to routes with parameters
        cx.update(|cx| {
            Navigator::push(cx, "/users/42");
        });

        assert_eq!(cx.read(Navigator::current_path), "/users/42");

        cx.update(|cx| {
            Navigator::push(cx, "/posts/123/comments/456");
        });

        assert_eq!(cx.read(Navigator::current_path), "/posts/123/comments/456");
    }

    #[gpui::test]
    fn test_navigator_api_style(cx: &mut TestAppContext) {
        // Initialize router
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

        // Test Flutter-style Navigator.of(context).push()
        cx.update(|cx| {
            Navigator::of(cx).push("/home");
        });

        assert_eq!(cx.read(Navigator::current_path), "/home");

        // Test chaining
        cx.update(|cx| {
            Navigator::of(cx).push("/profile").pop();
        });

        assert_eq!(cx.read(Navigator::current_path), "/home");

        // Test replace
        cx.update(|cx| {
            Navigator::of(cx).replace("/profile");
        });

        assert_eq!(cx.read(Navigator::current_path), "/profile");

        // After replace, we're still at index 1 in history, so we can still go back to "/"
        assert!(cx.read(Navigator::can_pop));

        // Pop back to "/"
        cx.update(|cx| {
            Navigator::of(cx).pop();
        });

        assert_eq!(cx.read(Navigator::current_path), "/");

        // Now we're at the root, can't go back anymore
        assert!(!cx.read(Navigator::can_pop));
    }

    #[gpui::test]
    fn test_material_route_with_params(cx: &mut TestAppContext) {
        use crate::PageRoute;

        // Initialize router
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::new("/", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
                router.add_route(Route::new("/users/:id", |_, _cx, _params| {
                    gpui::div().into_any_element()
                }));
            });
        });

        // Test PageRoute with params
        cx.update(|cx| {
            Navigator::push(
                cx,
                PageRoute::builder("/users/:id", |_, _cx, _params| {
                    gpui::div().into_any_element()
                })
                .with_param("id", "123"),
            );
        });

        assert_eq!(cx.read(Navigator::current_path), "/users/:id");

        // Test with Navigator.of() style
        cx.update(|cx| {
            Navigator::of(cx).push(
                PageRoute::builder("/users/:id", |_, _cx, _params| {
                    gpui::div().into_any_element()
                })
                .with_param("id", "456"),
            );
        });

        assert_eq!(cx.read(Navigator::current_path), "/users/:id");
    }

    #[gpui::test]
    fn test_string_into_route(cx: &mut TestAppContext) {
        // Initialize router
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

        // Test that strings still work with IntoRoute
        cx.update(|cx| {
            Navigator::push(cx, "/home");
        });

        assert_eq!(cx.read(Navigator::current_path), "/home");

        // Test with &str
        cx.update(|cx| {
            let path = "/home";
            Navigator::push(cx, path);
        });

        assert_eq!(cx.read(Navigator::current_path), "/home");

        // Test String
        cx.update(|cx| {
            Navigator::push(cx, String::from("/home"));
        });

        assert_eq!(cx.read(Navigator::current_path), "/home");
    }

    #[gpui::test]
    fn test_both_api_styles(cx: &mut TestAppContext) {
        // Initialize router
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

        // Use static API
        cx.update(|cx| {
            Navigator::push(cx, "/page1");
        });
        assert_eq!(cx.read(Navigator::current_path), "/page1");

        // Use Flutter-style API
        cx.update(|cx| {
            Navigator::of(cx).push("/page2");
        });
        assert_eq!(cx.read(Navigator::current_path), "/page2");

        // Mix both styles
        cx.update(|cx| {
            Navigator::pop(cx); // Static API
        });
        assert_eq!(cx.read(Navigator::current_path), "/page1");

        cx.update(|cx| {
            Navigator::of(cx).pop(); // Flutter style
        });
        assert_eq!(cx.read(Navigator::current_path), "/");
    }
}
