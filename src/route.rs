//! Route definition and configuration.
//!
//! This module is the core of the routing system. It provides:
//!
//! - [`Route`] — a route with a path pattern, builder function, children, guards,
//!   middleware, lifecycle hooks, and transitions.
//! - [`RouteConfig`] — lightweight configuration (path, name, metadata) without
//!   a builder or feature-gated fields.
//! - [`NamedRouteRegistry`] — a registry that maps human-readable names to path
//!   patterns for reverse URL generation.
//! - [`PageRoute`], [`NamedRoute`], and the [`IntoRoute`] trait — flexible
//!   navigation descriptors accepted by `Navigator::push`.
//!
//! # Route creation
//!
//! Routes are created via constructor methods on [`Route`]:
//!
//! | Constructor | Use case |
//! |-------------|----------|
//! | [`Route::new`] | Full control — receives `Window`, `App`, `RouteParams` |
//! | [`Route::view`] | Stateless page — simple closure returning `AnyElement` |
//! | [`Route::component`] | Stateful page — `Entity<T>` cached across navigations |
//! | [`Route::component_with_params`] | Stateful page keyed by parameters |
//!
//! # Builder pattern
//!
//! After construction, routes are configured using a fluent builder API:
//!
//! ```no_run
//! use gpui_navigator::{Route, render_router_outlet};
//! use gpui::*;
//!
//! let route = Route::new("/dashboard", |window, cx, params| {
//!         div()
//!             .child("Dashboard")
//!             .child(render_router_outlet(window, cx, None))
//!             .into_any_element()
//!     })
//!     .name("dashboard")
//!     .meta("title", "Dashboard")
//!     .children(vec![
//!         Route::new("", |_, _cx, _p| div().child("Overview").into_any_element()).into(),
//!         Route::new("settings", |_, _cx, _p| div().child("Settings").into_any_element()).into(),
//!     ]);
//! ```
//!
//! # Sharing routes
//!
//! [`Route`] contains non-cloneable fields (guards, middleware, lifecycle hooks).
//! Use [`RouteRef`] (`Arc<Route>`) to share routes cheaply across the route tree.

#[cfg(feature = "guard")]
use crate::guards::RouteGuard;
use crate::lifecycle::RouteLifecycle;
#[cfg(feature = "middleware")]
use crate::middleware::RouteMiddleware;
use crate::params::RouteParams;
#[cfg(feature = "transition")]
use crate::transition::TransitionConfig;
use crate::{trace_log, warn_log, RouteMatch};
use gpui::{AnyElement, AnyView, App, AppContext, BorrowAppContext, IntoElement, Render, Window};
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// NamedRouteRegistry
// ============================================================================

/// Registry that maps human-readable route names to path patterns.
///
/// Enables reverse URL generation: given a name and parameters, produce the
/// concrete path. This decouples link targets from literal paths, so renaming
/// a path only requires updating the route definition.
///
/// # Example
///
/// ```
/// use gpui_navigator::{NamedRouteRegistry, RouteParams};
///
/// let mut registry = NamedRouteRegistry::new();
/// registry.register("user.profile", "/users/:id/profile");
///
/// let mut params = RouteParams::new();
/// params.set("id".to_string(), "42".to_string());
///
/// assert_eq!(registry.url_for("user.profile", &params), Some("/users/42/profile".to_string()));
/// ```
#[derive(Clone, Debug, Default)]
pub struct NamedRouteRegistry {
    /// Map of route names to path patterns
    routes: HashMap<String, String>,
}

impl NamedRouteRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a named route
    pub fn register(&mut self, name: impl Into<String>, path: impl Into<String>) {
        self.routes.insert(name.into(), path.into());
    }

    /// Get path pattern for a named route
    pub fn get(&self, name: &str) -> Option<&str> {
        self.routes.get(name).map(|s| s.as_str())
    }

    /// Check if a route name exists
    pub fn contains(&self, name: &str) -> bool {
        self.routes.contains_key(name)
    }

    /// Generate URL for a named route with parameters
    ///
    /// # Example
    ///
    /// ```
    /// use gpui_navigator::{NamedRouteRegistry, RouteParams};
    ///
    /// let mut registry = NamedRouteRegistry::new();
    /// registry.register("user.detail", "/users/:id");
    ///
    /// let mut params = RouteParams::new();
    /// params.set("id".to_string(), "123".to_string());
    ///
    /// let url = registry.url_for("user.detail", &params).unwrap();
    /// assert_eq!(url, "/users/123");
    /// ```
    pub fn url_for(&self, name: &str, params: &RouteParams) -> Option<String> {
        let pattern = self.get(name)?;
        Some(substitute_params(pattern, params))
    }

    /// Clear all registered routes
    pub fn clear(&mut self) {
        self.routes.clear();
    }

    /// Get number of registered routes
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

/// Substitute route parameters in a path pattern
///
/// Replaces `:param` with actual values from RouteParams
fn substitute_params(pattern: &str, params: &RouteParams) -> String {
    let mut result = pattern.to_string();

    // Replace :param with actual values
    for (key, value) in params.iter() {
        let placeholder = format!(":{}", key);
        result = result.replace(&placeholder, value);
    }

    result
}

// ============================================================================
// Route Validation
// ============================================================================

/// Validate a route path pattern
///
/// Returns an error message if the path is invalid, None otherwise.
///
/// # Validation Rules
///
/// - Path can be empty (for index routes)
/// - Path must start with '/' or be relative (no leading '/')
/// - No consecutive slashes ('//')
/// - Trailing slashes are allowed but not recommended (normalized internally)
/// - Parameter names must be alphanumeric and not empty
/// - No duplicate parameter names
pub fn validate_route_path(path: &str) -> Result<(), String> {
    // Empty path is allowed for index routes
    if path.is_empty() {
        return Ok(());
    }

    // Consecutive slashes check
    if path.contains("//") {
        warn_log!("Invalid route path '{}': consecutive slashes", path);
        return Err("Route path cannot contain consecutive slashes".to_string());
    }

    // Note: Trailing slashes are allowed for compatibility
    // They are normalized during route matching

    // Extract and validate parameters
    let mut param_names = std::collections::HashSet::new();
    for segment in path.split('/') {
        if let Some(param) = segment.strip_prefix(':') {
            // Check parameter name is not empty
            if param.is_empty() {
                warn_log!("Invalid route path '{}': empty parameter name", path);
                return Err("Route parameter name cannot be empty".to_string());
            }

            // Check for constraint syntax (:id{uuid})
            let param_name = if let Some(pos) = param.find('{') {
                &param[..pos]
            } else {
                param
            };

            // Check parameter name is alphanumeric
            if !param_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                warn_log!(
                    "Invalid route path '{}': parameter '{}' has invalid characters",
                    path,
                    param_name
                );
                return Err(format!(
                    "Route parameter '{}' must contain only alphanumeric characters and underscores",
                    param_name
                ));
            }

            // Check for duplicate parameters
            if !param_names.insert(param_name.to_string()) {
                warn_log!(
                    "Invalid route path '{}': duplicate parameter '{}'",
                    path,
                    param_name
                );
                return Err(format!("Duplicate route parameter: '{}'", param_name));
            }
        }
    }

    Ok(())
}

// ============================================================================
// RouteConfig
// ============================================================================

/// Lightweight route configuration without a builder or feature-gated fields.
///
/// Carries the path pattern, optional name, child configs, and metadata.
/// Used internally by [`Route`] and also available standalone for scenarios
/// where you need route metadata without a render function.
#[derive(Debug, Clone)]
pub struct RouteConfig {
    /// Route path pattern (e.g., "/users/:id")
    pub path: String,
    /// Route name (optional)
    pub name: Option<String>,
    /// Child routes (NOTE: For nested routing, use Route.children() instead)
    pub children: Vec<RouteConfig>,
    /// Route metadata
    pub meta: HashMap<String, String>,
}

impl RouteConfig {
    /// Check if this is a layout route (has children but no explicit builder)
    pub fn is_layout(&self) -> bool {
        !self.children.is_empty()
    }
}

impl RouteConfig {
    /// Create a new route with path validation
    ///
    /// # Panics
    ///
    /// Panics if the path is invalid. Use `try_new` for non-panicking validation.
    pub fn new(path: impl Into<String>) -> Self {
        let path_str = path.into();
        if let Err(e) = validate_route_path(&path_str) {
            panic!("Invalid route path '{}': {}", path_str, e);
        }
        Self {
            path: path_str,
            name: None,
            children: Vec::new(),
            meta: HashMap::new(),
        }
    }

    /// Create a new route with validation, returning Result
    ///
    /// Use this if you want to handle validation errors instead of panicking.
    pub fn try_new(path: impl Into<String>) -> Result<Self, String> {
        let path_str = path.into();
        validate_route_path(&path_str)?;
        Ok(Self {
            path: path_str,
            name: None,
            children: Vec::new(),
            meta: HashMap::new(),
        })
    }

    /// Set route name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add child routes
    pub fn children(mut self, children: Vec<RouteConfig>) -> Self {
        self.children = children;
        self
    }

    /// Add a child route
    pub fn child(mut self, child: RouteConfig) -> Self {
        self.children.push(child);
        self
    }

    /// Add metadata
    pub fn meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.meta.insert(key.into(), value.into());
        self
    }
}

/// Type for route builder function
///
/// Builder receives Window, context and parameters, returns an AnyElement.
/// Through context you have access to App, global state, and Navigator.
///
/// The `Window` parameter allows builders to use stateful GPUI features like
/// `window.use_state()` or `window.use_keyed_state()` for caching expensive computations
/// or creating persistent entities across renders.
///
/// Note: When using `Route::new()`, your builder can return any type that implements
/// `IntoElement` - the conversion to `AnyElement` is done automatically.
pub type RouteBuilder =
    Arc<dyn Fn(&mut Window, &mut App, &RouteParams) -> AnyElement + Send + Sync>;

/// Shared route handle.
///
/// A `Route` contains non-cloneable behavior (guards/middleware/lifecycle).
/// To make route trees cheap to share and cache, the canonical way to pass
/// routes around is via `Arc<Route>`.
pub type RouteRef = Arc<Route>;

/// Look up a cached component view by `key`, or create and cache a new one.
///
/// Used by [`Route::component`] and [`Route::component_with_params`] to
/// avoid duplicating the cache-check/create/store pattern.
fn get_or_create_cached_component<T: Render + 'static>(
    cx: &mut App,
    key: String,
    create: impl FnOnce() -> T,
) -> AnyElement {
    // Check the global component cache first (survives across navigations)
    if let Some(router) = cx.try_global::<crate::context::GlobalRouter>() {
        if let Some(cached) = router.get_cached_component(&key) {
            return cached.clone().into_any_element();
        }
    }

    // Not cached — create a new entity and cache it
    let entity: gpui::Entity<T> = cx.new(|_| create());
    let view: AnyView = entity.into();

    if cx.try_global::<crate::context::GlobalRouter>().is_some() {
        cx.update_global::<crate::context::GlobalRouter, _>(
            |router: &mut crate::context::GlobalRouter, _| {
                router.cache_component(key, view.clone());
            },
        );
    }

    view.into_any_element()
}

/// A single route in the navigation tree.
///
/// Combines a path pattern, an optional builder function, child routes, and
/// feature-gated behavior (guards, middleware, lifecycle hooks, transitions).
///
/// Create routes with [`Route::new`], [`Route::view`], [`Route::component`],
/// or [`Route::component_with_params`], then configure them with the fluent
/// builder methods.
pub struct Route {
    /// Route configuration
    pub config: RouteConfig,
    /// Builder function to create the view for this route
    pub builder: Option<RouteBuilder>,
    /// Child routes with their own builders
    /// This is the preferred way to define nested routes (instead of RouteConfig.children)
    pub children: Vec<RouteRef>,
    /// Named outlets - map of outlet name to child routes
    /// Allows multiple outlet areas in a single parent route
    pub named_children: HashMap<String, Vec<RouteRef>>,
    /// Guards that control access to this route
    #[cfg(feature = "guard")]
    pub guards: Vec<Box<dyn RouteGuard>>,
    /// Middleware that runs before and after navigation to this route
    #[cfg(feature = "middleware")]
    pub middleware: Vec<Box<dyn RouteMiddleware>>,
    /// Lifecycle hooks for this route
    pub lifecycle: Option<Box<dyn RouteLifecycle>>,
    /// Transition animation for this route
    #[cfg(feature = "transition")]
    pub transition: TransitionConfig,
}

impl Route {
    /// Create a route with a builder function.
    ///
    /// The builder receives the window, app context and extracted route parameters,
    /// and must return `AnyElement`. Use `.into_any_element()` on your element.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::Route;
    /// use gpui::*;
    ///
    /// // Simple static route
    /// Route::new("/home", |_window, _cx, _params| {
    ///     div().child("Home Page").into_any_element()
    /// });
    ///
    /// // Route with dynamic parameter
    /// Route::new("/users/:id", |_window, _cx, params| {
    ///     let id = params.get("id").unwrap();
    ///     div().child(format!("User: {}", id)).into_any_element()
    /// });
    /// ```
    pub fn new<F>(path: impl Into<String>, builder: F) -> Self
    where
        F: Fn(&mut Window, &mut App, &RouteParams) -> AnyElement + Send + Sync + 'static,
    {
        Self {
            config: RouteConfig::new(path),
            builder: Some(Arc::new(builder)),
            children: Vec::new(),
            named_children: HashMap::new(),
            #[cfg(feature = "guard")]
            guards: Vec::new(),
            #[cfg(feature = "middleware")]
            middleware: Vec::new(),
            lifecycle: None,
            #[cfg(feature = "transition")]
            transition: TransitionConfig::default(),
        }
    }

    /// Create a stateless route from a simple view function.
    ///
    /// Use this for simple, stateless pages that don't need access to route params,
    /// window, or context. The view function is called on every render.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::Route;
    /// use gpui::*;
    ///
    /// Route::view("/about", || {
    ///     div().child("About Page").into_any_element()
    /// });
    /// ```
    pub fn view(
        path: impl Into<String>,
        view: impl Fn() -> AnyElement + Send + Sync + 'static,
    ) -> Self {
        Self::new(path, move |_, _, _| view())
    }

    /// Create a stateful route with an Entity-based component
    ///
    /// Use this for pages that maintain internal state across navigation.
    /// The component is cached using `window.use_keyed_state()`, so navigating
    /// back to the route will preserve the component's state.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::Route;
    /// use gpui::*;
    ///
    /// struct CounterPage {
    ///     count: i32,
    /// }
    ///
    /// impl CounterPage {
    ///     fn new() -> Self {
    ///         Self { count: 0 }
    ///     }
    /// }
    ///
    /// impl Render for CounterPage {
    ///     fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
    ///         div().child(format!("Count: {}", self.count))
    ///     }
    /// }
    ///
    /// Route::component("/counter", CounterPage::new);
    /// ```
    pub fn component<T, F>(path: impl Into<String>, create: F) -> Self
    where
        T: Render + 'static,
        F: Fn() -> T + Send + Sync + 'static + Clone,
    {
        let path_str = path.into();
        let key_path = path_str.clone();
        let type_id = std::any::TypeId::of::<T>();

        Self::new(path_str, move |_window, cx, _| {
            let key = format!("route:{}:{:?}", key_path, type_id);
            let create_fn = create.clone();
            get_or_create_cached_component(cx, key, create_fn)
        })
    }

    /// Create a stateful route with parameters
    ///
    /// Like `component()`, but the create function receives route parameters.
    /// The component is cached per unique set of parameter values, so navigating
    /// to `/user/1` and `/user/2` will create two separate component instances.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::{Route, RouteParams};
    /// use gpui::*;
    ///
    /// struct UserPage {
    ///     user_id: String,
    /// }
    ///
    /// impl UserPage {
    ///     fn new(user_id: String) -> Self {
    ///         Self { user_id }
    ///     }
    /// }
    ///
    /// impl Render for UserPage {
    ///     fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
    ///         div().child(format!("User: {}", self.user_id))
    ///     }
    /// }
    ///
    /// Route::component_with_params("/user/:id", |params| {
    ///     let id = params.get("id").unwrap().to_string();
    ///     UserPage::new(id)
    /// });
    /// ```
    pub fn component_with_params<T, F>(path: impl Into<String>, create: F) -> Self
    where
        T: Render + 'static,
        F: Fn(&RouteParams) -> T + Send + Sync + 'static + Clone,
    {
        let path_str = path.into();
        let key_path = path_str.clone();
        let type_id = std::any::TypeId::of::<T>();

        Self::new(path_str, move |_window, cx, params| {
            let params_key = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            let key = format!("route:{}:{:?}?{}", key_path, type_id, params_key);
            let params_clone = params.clone();
            let create_fn = create.clone();
            get_or_create_cached_component(cx, key, || create_fn(&params_clone))
        })
    }

    /// Add child routes to this route
    ///
    /// Child routes will be rendered in a RouterOutlet within the parent's layout.
    ///
    /// # T038: Index Route Validation
    ///
    /// This method validates child routes and warns if there's an ambiguous default:
    /// - If a parent has multiple children and no index route (path=""), users must
    ///   explicitly navigate to a child path. Without an index, navigating to the
    ///   parent path alone will show nothing.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::{Route, render_router_outlet};
    /// use gpui::*;
    ///
    /// // Good: Has index route for default content
    /// Route::new("/dashboard", |window, cx, params| {
    ///     div()
    ///         .child("Dashboard Header")
    ///         .child(render_router_outlet(window, cx, None))
    ///         .into_any_element()
    /// })
    /// .children(vec![
    ///     Route::new("", |_, _cx, _params| {        // Index route
    ///         div().child("Overview (Default)").into_any_element()
    ///     })
    ///     .into(),
    ///     Route::new("settings", |_, _cx, _params| {
    ///         div().child("Settings").into_any_element()
    ///     })
    ///     .into(),
    /// ]);
    ///
    /// // Warning: No index route - navigating to "/dashboard" shows nothing
    /// Route::new("/dashboard", |window, cx, params| {
    ///     div()
    ///         .child("Dashboard Header")
    ///         .child(render_router_outlet(window, cx, None))
    ///         .into_any_element()
    /// })
    /// .children(vec![
    ///     Route::new("overview", |_, _cx, _params| {
    ///         div().child("Overview").into_any_element()
    ///     })
    ///     .into(),
    ///     Route::new("settings", |_, _cx, _params| {
    ///         div().child("Settings").into_any_element()
    ///     })
    ///     .into(),
    /// ]);
    /// ```
    pub fn children(mut self, children: Vec<RouteRef>) -> Self {
        // T038: Validate index routes - warn if ambiguous default
        if children.len() > 1 {
            let has_index = children.iter().any(|child| {
                let path = child.config.path.trim_matches('/');
                path.is_empty() || path == "index"
            });

            if !has_index && !self.config.path.is_empty() {
                warn_log!(
                    "Route '{}' has {} children but no index route (path=\"\" or \"index\"). \
                     Navigating to '{}' alone will show no content.",
                    self.config.path,
                    children.len(),
                    self.config.path
                );
            }
        }

        self.children = children;
        self
    }

    /// Add a single child route
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::Route;
    /// use gpui::*;
    ///
    /// Route::new("/dashboard", |_, _cx, _params| div().into_any_element())
    ///     .child(Route::new("overview", |_, _cx, _params| div().into_any_element()).into())
    ///     .child(Route::new("settings", |_, _cx, _params| div().into_any_element()).into());
    /// ```
    pub fn child(mut self, child: RouteRef) -> Self {
        self.children.push(child);
        self
    }

    /// Set route name
    ///
    /// Named routes can be referenced by name instead of path.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.config.name = Some(name.into());
        self
    }

    /// Add metadata to the route
    ///
    /// Metadata can be used for guards, analytics, titles, etc.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::Route;
    /// use gpui::*;
    ///
    /// Route::new("/admin", |_, _cx, _params| div().into_any_element())
    ///     .meta("requiresAuth", "true")
    ///     .meta("requiredRole", "admin")
    ///     .meta("title", "Admin Panel");
    /// ```
    pub fn meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.meta.insert(key.into(), value.into());
        self
    }

    /// Add routes for a named outlet
    ///
    /// Named outlets allow you to have multiple content areas in a single parent route.
    /// For example, a main content area and a sidebar.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::{Route, render_router_outlet};
    /// use gpui::*;
    ///
    /// Route::new("/dashboard", |window, cx, _params| {
    ///     div()
    ///         .child(render_router_outlet(window, cx, None))             // Main content
    ///         .child(render_router_outlet(window, cx, Some("sidebar")))  // Sidebar
    ///         .into_any_element()
    /// })
    /// .children(vec![
    ///     Route::new("analytics", |_, _cx, _params| div().into_any_element()).into(),
    /// ])
    /// .named_outlet("sidebar", vec![
    ///     Route::new("stats", |_, _cx, _params| div().into_any_element()).into(),
    /// ]);
    /// ```
    pub fn named_outlet(mut self, name: impl Into<String>, children: Vec<RouteRef>) -> Self {
        self.named_children.insert(name.into(), children);
        self
    }

    /// Add a guard to this route
    ///
    /// Guards control access to routes. If any guard denies access, navigation is blocked.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::{Route, AuthGuard, RoleGuard};
    /// use gpui::*;
    ///
    /// fn is_authenticated(_cx: &App) -> bool { true }
    /// fn get_role(_cx: &App) -> Option<String> { Some("user".into()) }
    ///
    /// Route::new("/dashboard", |_, _cx, _params| div().into_any_element())
    ///     .guard(AuthGuard::new(is_authenticated, "/login"))
    ///     .guard(RoleGuard::new(get_role, "user", Some("/forbidden")));
    /// ```
    #[cfg(feature = "guard")]
    pub fn guard<G: crate::guards::RouteGuard>(mut self, guard: G) -> Self {
        self.guards.push(Box::new(guard));
        self
    }

    /// Add multiple guards at once (pre-boxed).
    #[cfg(feature = "guard")]
    pub fn guards(mut self, guards: Vec<Box<dyn crate::guards::RouteGuard>>) -> Self {
        self.guards.extend(guards);
        self
    }

    /// Add middleware to this route
    ///
    /// Middleware runs before and after navigation.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::Route;
    /// use gpui::*;
    ///
    /// // Route::new("/dashboard", |_, _cx, _params| div().into_any_element())
    /// //     .middleware(LoggingMiddleware::new());
    /// ```
    #[cfg(feature = "middleware")]
    pub fn middleware<M: crate::middleware::RouteMiddleware>(mut self, middleware: M) -> Self {
        self.middleware.push(Box::new(middleware));
        self
    }

    /// Add multiple middleware at once (pre-boxed).
    #[cfg(feature = "middleware")]
    pub fn middlewares(
        mut self,
        middleware: Vec<Box<dyn crate::middleware::RouteMiddleware>>,
    ) -> Self {
        self.middleware.extend(middleware);
        self
    }

    /// Add lifecycle hooks to this route
    ///
    /// Lifecycle hooks allow you to run code when entering/exiting routes.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui_navigator::{Route, RouteLifecycle, NavigationRequest};
    /// use gpui::*;
    /// use std::pin::Pin;
    /// use std::future::Future;
    ///
    /// // Lifecycle hooks allow running code when entering/exiting routes
    /// // Implement RouteLifecycle trait for custom behavior
    /// ```
    pub fn lifecycle<L: crate::lifecycle::RouteLifecycle>(mut self, lifecycle: L) -> Self {
        self.lifecycle = Some(Box::new(lifecycle));
        self
    }

    /// Set the transition animation for this route
    ///
    /// # Example
    /// ```no_run
    /// use gpui_navigator::{Route, Transition};
    /// use gpui::*;
    ///
    /// Route::new("/page", |_, _cx, _params| div().into_any_element())
    ///     .transition(Transition::fade(200));
    /// ```
    #[cfg(feature = "transition")]
    pub fn transition(mut self, transition: crate::transition::Transition) -> Self {
        self.transition = TransitionConfig::new(transition);
        self
    }

    /// Get child routes for a named outlet
    ///
    /// Returns None if the outlet doesn't exist
    pub fn get_named_children(&self, name: &str) -> Option<&[RouteRef]> {
        self.named_children.get(name).map(|v| v.as_slice())
    }

    /// Check if this route has a named outlet
    pub fn has_named_outlet(&self, name: &str) -> bool {
        self.named_children.contains_key(name)
    }

    /// Get all named outlet names
    pub fn named_outlet_names(&self) -> Vec<&str> {
        self.named_children.keys().map(|s| s.as_str()).collect()
    }

    /// Match a path against this route
    pub fn matches(&self, path: &str) -> Option<RouteMatch> {
        match_path(&self.config.path, path)
    }

    /// Build the view for this route
    pub fn build(
        &self,
        window: &mut Window,
        cx: &mut App,
        params: &RouteParams,
    ) -> Option<AnyElement> {
        trace_log!(
            "Building route '{}' with {} params",
            self.config.path,
            params.len()
        );
        self.builder.as_ref().map(|b| b(window, cx, params))
    }

    /// Find a child route by path segment
    ///
    /// Used internally by RouterOutlet to resolve child routes.
    pub fn find_child(&self, segment: &str) -> Option<&RouteRef> {
        self.children.iter().find(|child| {
            child.config.path == segment || child.config.path.trim_start_matches('/') == segment
        })
    }

    /// Get all child routes
    pub fn get_children(&self) -> &[RouteRef] {
        &self.children
    }
}

impl std::fmt::Debug for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Route")
            .field("config", &self.config)
            .field("builder", &self.builder.is_some())
            .field("children", &self.children.len())
            .field(
                "named_children",
                &self.named_children.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Match a path pattern against an actual path
///
/// Supports:
/// - Static paths: `/users`
/// - Dynamic segments: `/users/:id`
/// - Wildcard: `/files/*`
fn match_path(pattern: &str, path: &str) -> Option<RouteMatch> {
    let pattern_iter = pattern.split('/').filter(|s| !s.is_empty());
    let mut path_iter = path.split('/').filter(|s| !s.is_empty());

    let mut route_match = RouteMatch::new(path.to_string());

    for pattern_seg in pattern_iter {
        if pattern_seg == "*" {
            // Wildcard matches rest of path
            return Some(route_match);
        }

        let Some(path_seg) = path_iter.next() else {
            // Path exhausted before pattern — no match
            return None;
        };

        if let Some(param_name) = pattern_seg.strip_prefix(':') {
            route_match
                .params
                .insert(param_name.to_string(), path_seg.to_string());
        } else if pattern_seg != path_seg {
            return None;
        }
    }

    // All pattern segments consumed — path must also be exhausted
    if path_iter.next().is_some() {
        return None;
    }

    Some(route_match)
}

// ============================================================================
// Route Builder Utilities
// ============================================================================

//
// This module provides a flexible routing system that can accept both simple
// string paths and route builders with parameters.

/// Trait for types that can be converted into a route
///
/// This allows Navigator.push() to accept both strings and route builders:
/// ```ignore
/// use gpui_navigator::{Navigator, PageRoute};
///
/// // String path
/// Navigator::push(cx, "/users");
///
/// // Route with builder
/// Navigator::push(cx, PageRoute::builder("/profile", |_, _cx, _params| {
///     gpui::div()
/// }));
/// ```
pub trait IntoRoute {
    /// Convert this type into a route path and optional builder
    fn into_route(self) -> RouteDescriptor;
}

/// Type for route builder function
pub type BuilderFn = Arc<dyn Fn(&mut Window, &mut App, &RouteParams) -> AnyElement + Send + Sync>;

/// A route descriptor containing path, parameters, and optional builder
pub struct RouteDescriptor {
    /// The route path (e.g., "/users/:id")
    pub path: String,

    /// Parameters to pass to the route
    pub params: RouteParams,

    /// Optional builder function to create the view
    pub builder: Option<BuilderFn>,
}

// RouteParams is now imported from crate::params::RouteParams

// Implement IntoRoute for String (simple path navigation)
impl IntoRoute for String {
    fn into_route(self) -> RouteDescriptor {
        RouteDescriptor {
            path: self,
            params: RouteParams::new(),
            builder: None,
        }
    }
}

// Implement IntoRoute for &str
impl IntoRoute for &str {
    fn into_route(self) -> RouteDescriptor {
        RouteDescriptor {
            path: self.to_string(),
            params: RouteParams::new(),
            builder: None,
        }
    }
}

/// A page route with optional builder function
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::{Navigator, PageRoute};
///
/// // Simple path (no builder)
/// Navigator::push(cx, PageRoute::new("/users"));
///
/// // With parameters
/// Navigator::push(
///     cx,
///     PageRoute::new("/users/:id")
///         .with_param("id".into(), "123".into())
/// );
///
/// // With builder function
/// Navigator::push(
///     cx,
///     PageRoute::builder("/profile", |_window, cx, params| {
///         div().child("Profile Page")
///     })
/// );
/// ```
pub struct PageRoute {
    path: String,
    params: RouteParams,
    builder: Option<BuilderFn>,
}

impl PageRoute {
    /// Create a new PageRoute with a path (no builder)
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            params: RouteParams::new(),
            builder: None,
        }
    }

    /// Create a PageRoute with a builder function.
    ///
    /// The builder must return `AnyElement`.
    pub fn builder(
        path: impl Into<String>,
        builder: impl Fn(&mut Window, &mut App, &RouteParams) -> AnyElement + Send + Sync + 'static,
    ) -> Self {
        Self {
            path: path.into(),
            params: RouteParams::new(),
            builder: Some(Arc::new(builder)),
        }
    }

    /// Set the builder function for this route.
    ///
    /// The builder must return `AnyElement`.
    pub fn with_builder(
        mut self,
        builder: impl Fn(&mut Window, &mut App, &RouteParams) -> AnyElement + Send + Sync + 'static,
    ) -> Self {
        self.builder = Some(Arc::new(builder));
        self
    }

    /// Add a parameter to this route
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    /// Add multiple parameters
    pub fn with_params(mut self, params: HashMap<String, String>) -> Self {
        self.params = RouteParams::from_map(params);
        self
    }
}

impl IntoRoute for PageRoute {
    fn into_route(self) -> RouteDescriptor {
        RouteDescriptor {
            path: self.path,
            params: self.params,
            builder: self.builder,
        }
    }
}

/// A named route for predefined routes
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::{Navigator, NamedRoute, RouteParams};
///
/// let mut params = RouteParams::new();
/// params.set("userId".to_string(), "123".to_string());
/// Navigator::push_named(cx, "user_profile", &params);
/// ```
pub struct NamedRoute {
    name: String,
    params: RouteParams,
}

impl NamedRoute {
    /// Create a new named route
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            params: RouteParams::new(),
        }
    }

    /// Add a parameter
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    /// Add multiple parameters
    pub fn with_params(mut self, params: HashMap<String, String>) -> Self {
        self.params = RouteParams::from_map(params);
        self
    }
}

impl IntoRoute for NamedRoute {
    fn into_route(self) -> RouteDescriptor {
        RouteDescriptor {
            path: self.name,
            params: self.params,
            builder: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NamedRouteRegistry tests

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = NamedRouteRegistry::new();
        registry.register("home", "/");
        registry.register("user.detail", "/users/:id");

        assert_eq!(registry.get("home"), Some("/"));
        assert_eq!(registry.get("user.detail"), Some("/users/:id"));
        assert_eq!(registry.get("unknown"), None);
    }

    #[test]
    fn test_registry_contains() {
        let mut registry = NamedRouteRegistry::new();
        registry.register("home", "/");

        assert!(registry.contains("home"));
        assert!(!registry.contains("unknown"));
    }

    #[test]
    fn test_url_for_simple() {
        let mut registry = NamedRouteRegistry::new();
        registry.register("home", "/");

        let params = RouteParams::new();
        assert_eq!(registry.url_for("home", &params), Some("/".to_string()));
    }

    #[test]
    fn test_url_for_with_params() {
        let mut registry = NamedRouteRegistry::new();
        registry.register("user.detail", "/users/:id");

        let mut params = RouteParams::new();
        params.set("id".to_string(), "123".to_string());

        assert_eq!(
            registry.url_for("user.detail", &params),
            Some("/users/123".to_string())
        );
    }

    #[test]
    fn test_url_for_multiple_params() {
        let mut registry = NamedRouteRegistry::new();
        registry.register("post.comment", "/posts/:postId/comments/:commentId");

        let mut params = RouteParams::new();
        params.set("postId".to_string(), "42".to_string());
        params.set("commentId".to_string(), "99".to_string());

        assert_eq!(
            registry.url_for("post.comment", &params),
            Some("/posts/42/comments/99".to_string())
        );
    }

    #[test]
    fn test_url_for_unknown_route() {
        let registry = NamedRouteRegistry::new();
        let params = RouteParams::new();

        assert_eq!(registry.url_for("unknown", &params), None);
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = NamedRouteRegistry::new();
        registry.register("home", "/");
        registry.register("about", "/about");

        assert_eq!(registry.len(), 2);

        registry.clear();

        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_substitute_params() {
        let mut params = RouteParams::new();
        params.set("id".to_string(), "123".to_string());
        params.set("action".to_string(), "edit".to_string());

        let result = substitute_params("/users/:id/:action", &params);
        assert_eq!(result, "/users/123/edit");
    }

    // Route tests

    #[test]
    fn test_static_route() {
        let result = match_path("/users", "/users");
        assert!(result.is_some());

        let result = match_path("/users", "/posts");
        assert!(result.is_none());
    }

    #[test]
    fn test_dynamic_route() {
        let result = match_path("/users/:id", "/users/123");
        assert!(result.is_some());

        let route_match = result.unwrap();
        assert_eq!(route_match.params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_wildcard_route() {
        let result = match_path("/files/*", "/files/documents/report.pdf");
        assert!(result.is_some());

        let result = match_path("/files/*", "/other/path");
        assert!(result.is_none());
    }

    #[test]
    fn test_string_into_route() {
        let route = "/users".into_route();
        assert_eq!(route.path, "/users");
        assert!(route.params.all().is_empty());
    }

    #[test]
    fn test_material_route_with_params() {
        let route = PageRoute::new("/users/:id")
            .with_param("id", "123")
            .into_route();

        assert_eq!(route.path, "/users/:id");
        assert_eq!(route.params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_named_route() {
        let route = NamedRoute::new("user_profile")
            .with_param("userId", "456")
            .into_route();

        assert_eq!(route.path, "user_profile");
        assert_eq!(route.params.get("userId"), Some(&"456".to_string()));
    }

    // Validation tests

    #[test]
    fn test_validate_valid_paths() {
        assert!(validate_route_path("/").is_ok());
        assert!(validate_route_path("/users").is_ok());
        assert!(validate_route_path("/users/:id").is_ok());
        assert!(validate_route_path("/posts/:postId/comments/:commentId").is_ok());
        assert!(validate_route_path("/users/:id{uuid}").is_ok());
        assert!(validate_route_path("/api/v1/users").is_ok());
        assert!(validate_route_path("settings").is_ok()); // relative path
        assert!(validate_route_path("").is_ok()); // empty path (index route)
        assert!(validate_route_path("/users/").is_ok()); // trailing slash allowed
    }

    #[test]
    fn test_validate_consecutive_slashes() {
        let result = validate_route_path("/users//profile");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("consecutive slashes"));
    }

    #[test]
    fn test_validate_empty_parameter() {
        let result = validate_route_path("/users/:");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("parameter name cannot be empty"));
    }

    #[test]
    fn test_validate_invalid_parameter_name() {
        let result = validate_route_path("/users/:user-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("alphanumeric"));
    }

    #[test]
    fn test_validate_duplicate_parameters() {
        let result = validate_route_path("/users/:id/posts/:id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate"));
    }

    #[test]
    fn test_route_config_try_new_valid() {
        let result = RouteConfig::try_new("/users/:id");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().path, "/users/:id");
    }

    #[test]
    fn test_route_config_try_new_invalid() {
        let result = RouteConfig::try_new("/users//profile");
        assert!(result.is_err());
    }

    #[test]
    #[should_panic(expected = "Invalid route path")]
    fn test_route_config_new_panics_on_invalid() {
        RouteConfig::new("/users//profile");
    }
}
