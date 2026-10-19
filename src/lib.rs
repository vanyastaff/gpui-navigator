//! # GPUI Router
//!
//! A declarative routing library for GPUI with support for:
//!
//! - **Route Transitions** - Fade, slide, scale animations with configurable duration
//! - **Nested Routing** - Parent/child route hierarchies with `RouterOutlet`
//! - **Route Guards** - Authentication, authorization, and custom access control
//! - **Middleware** - Before/after hooks for navigation events
//! - **Named Routes** - Navigate using route names instead of paths
//! - **Route Matching** - Pattern matching with parameters and constraints
//! - **Error Handling** - Custom 404 and error handlers
//!
//! # Quick Start
//!
//! ```ignore
//! use gpui::*;
//! use gpui_navigator::*;
//!
//! fn main() {
//!     Application::new().run(|cx| {
//!         init_router(cx, |router| {
//!             router.add_route(
//!                 Route::new("/", home_page)
//!                     .transition(Transition::fade(300))
//!             );
//!         });
//!
//!         cx.open_window(WindowOptions::default(), |_, cx| {
//!             cx.new(|_| AppView)
//!         })
//!     });
//! }
//!
//! fn home_page(_cx: &mut App, _params: &RouteParams) -> AnyElement {
//!     gpui::div().into_any_element()
//! }
//!
//! struct AppView;
//!
//! impl Render for AppView {
//!     fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
//!         gpui::div()
//!     }
//! }
//! ```
//!
//! # Navigation
//!
//! The library provides a simple navigation API:
//!
//! ```ignore
//! use gpui_navigator::Navigator;
//!
//! // Push new route
//! Navigator::push(cx, "/profile");
//!
//! // Replace current route
//! Navigator::replace(cx, "/login");
//!
//! // Go back (pop)
//! Navigator::pop(cx);
//!
//! // Go forward
//! Navigator::forward(cx);
//! ```
//!
//! # Route Guards
//!
//! Protect routes with authentication and authorization:
//!
//! ```no_run
//! use gpui_navigator::*;
//!
//! Route::new("/admin", admin_page)
//!     .guard(AuthGuard::new(is_logged_in, "/login"))
//!     .guard(RoleGuard::new(get_user_role, "admin", Some("/forbidden")))
//! # ;
//! # fn admin_page(_: &mut gpui::Window, _: &mut gpui::App, _: &RouteParams) -> gpui::AnyElement { todo!() }
//! # fn is_logged_in(_: &gpui::App) -> bool { todo!() }
//! # fn get_user_role(_: &gpui::App) -> Option<String> { todo!() }
//! ```
//!
//! # Nested Routes
//!
//! Create hierarchical route structures:
//!
//! ```no_run
//! use gpui_navigator::*;
//!
//! Route::new("/dashboard", dashboard_layout)
//!     .children(vec![
//!         Route::new("overview", overview_page).into(),
//!         Route::new("settings", settings_page).into(),
//!     ])
//! # ;
//! # fn dashboard_layout(_: &mut gpui::Window, _: &mut gpui::App, _: &RouteParams) -> gpui::AnyElement { todo!() }
//! # fn overview_page(_: &mut gpui::Window, _: &mut gpui::App, _: &RouteParams) -> gpui::AnyElement { todo!() }
//! # fn settings_page(_: &mut gpui::Window, _: &mut gpui::App, _: &RouteParams) -> gpui::AnyElement { todo!() }
//! ```
//!
//! # Feature Flags
//!
//! | Feature      | Default | Description |
//! |--------------|---------|-------------|
//! | `guard`      | yes     | Route guards (`AuthGuard`, `RoleGuard`, `PermissionGuard`) |
//! | `middleware`  | yes     | Before/after navigation hooks |
//! | `transition` | yes     | Fade, slide animations on route change |
//! | `cache`      | yes     | LRU cache for route resolution (depends on `lru`) |
//! | `log`        | yes     | Logging via the `log` crate |
//! | `tracing`    | no      | Logging via `tracing` (mutually exclusive with `log`) |
//!
//! # GPUI Backends
//!
//! Exactly one backend feature must be enabled. Pick the one that matches the
//! `gpui` your app (and `gpui-component`, if you use it) depends on:
//!
//! | Feature     | Default | Backend crate | Used by |
//! |-------------|---------|---------------|---------|
//! | `gpui-02`   | yes     | `gpui` 0.2    | gpui-component 0.5.x |
//! | `gpui-pre`  | no      | `gpui-pre`    | gpui-component 0.6.x |
//! | `gpui-fast` | no      | `gpui-fast`   | gpui-component 0.7.x |
//!
//! ```toml
//! gpui-navigator = { version = "0.2", default-features = false, features = ["gpui-pre", "transition"] }
//! ```

#![doc(html_root_url = "https://docs.rs/gpui_navigator/0.1.4")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
// Lints are configured in Cargo.toml [lints] section

// GPUI backend selection. Every backend crate names its library `gpui`; the
// non-default ones are renamed in Cargo.toml and aliased back here so the rest
// of the crate can keep using `gpui::...` paths.
#[cfg(not(any(feature = "gpui-02", feature = "gpui-pre", feature = "gpui-fast")))]
compile_error!(
    "gpui-navigator: enable one GPUI backend feature: `gpui-02` (default), `gpui-pre` or `gpui-fast`"
);
#[cfg(any(
    all(feature = "gpui-02", feature = "gpui-pre"),
    all(feature = "gpui-02", feature = "gpui-fast"),
    all(feature = "gpui-pre", feature = "gpui-fast"),
))]
compile_error!(
    "gpui-navigator: the GPUI backend features `gpui-02`, `gpui-pre` and `gpui-fast` are mutually exclusive; \
     add `default-features = false` when selecting `gpui-pre` or `gpui-fast`"
);
#[cfg(all(
    feature = "gpui-fast",
    not(feature = "gpui-02"),
    not(feature = "gpui-pre")
))]
extern crate gpui_fast as gpui;
#[cfg(all(feature = "gpui-pre", not(feature = "gpui-02")))]
extern crate gpui_pre as gpui;

// Logging abstraction
pub mod logging;

// Cache (optional)
#[cfg(feature = "cache")]
pub mod cache;

// Core routing modules
pub mod history;
#[cfg(feature = "middleware")]
pub mod middleware;
pub mod route;
pub mod state;

// Error handling
pub mod error;

// Route lifecycle
pub mod lifecycle;

// Guards
#[cfg(feature = "guard")]
pub mod guards;

// Transitions
#[cfg(feature = "transition")]
pub mod transition;

// Other modules
pub mod nested;
pub mod params;
pub mod resolve;
pub mod widgets;

// Context module (router context integration)
mod context;

// Re-export main types for convenient access
#[cfg(feature = "cache")]
pub use cache::{CacheStats, RouteCache, RouteId};
pub use context::{
    current_path, init_router, navigate, GlobalRouter, NavigationRequest, Navigator,
    NavigatorHandle, UseRouter,
};
pub use error::{ErrorHandler, ErrorHandlers, NavigationError, NavigationResult, NotFoundHandler};
#[cfg(feature = "guard")]
pub use guards::{
    guard_fn, AuthGuard, GuardBuilder, Guards, NotGuard, PermissionGuard, RoleGuard, RouteGuard,
};
pub use history::{History, HistoryEntry, HistoryState};
pub use lifecycle::{NavigationAction, RouteLifecycle};
#[cfg(feature = "middleware")]
pub use middleware::{middleware_fn, RouteMiddleware};
pub use nested::{build_child_path, extract_param_name, normalize_path, resolve_child_route};
pub use params::{QueryParams, RouteParams};
pub use resolve::{resolve_match_stack, MatchEntry, MatchStack};
pub use route::{
    validate_route_path, BuilderFn, IntoRoute, NamedRoute, NamedRouteRegistry, PageRoute, Route,
    RouteConfig, RouteDescriptor,
};
pub use state::RouterState;
#[cfg(feature = "transition")]
pub use transition::{SlideDirection, Transition, TransitionConfig};
pub use widgets::{
    render_router_outlet, router_link, router_outlet, router_outlet_named, router_view,
    DefaultPages, RouterLink, RouterOutlet, RouterView,
};

use std::collections::HashMap;

/// Route path matching result.
///
/// Contains the matched path along with any extracted parameters and query strings.
///
/// # Example
///
/// ```
/// use gpui_navigator::RouteMatch;
///
/// let route_match = RouteMatch::new("/users/123".to_string())
///     .with_param("id".to_string(), "123".to_string());
///
/// assert_eq!(route_match.params.get("id"), Some(&"123".to_string()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteMatch {
    /// The matched path
    pub path: String,
    /// Extracted route parameters (e.g., `:id` -> "123")
    pub params: HashMap<String, String>,
    /// Parsed query string parameters
    pub query: HashMap<String, String>,
}

impl RouteMatch {
    /// Create a new route match with the given path.
    #[must_use]
    pub fn new(path: String) -> Self {
        Self {
            path,
            params: HashMap::new(),
            query: HashMap::new(),
        }
    }

    /// Add a route parameter to the match.
    #[must_use]
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    /// Add a query parameter to the match.
    #[must_use]
    pub fn with_query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.insert(key.into(), value.into());
        self
    }
}

/// Navigation direction indicator.
///
/// Used to determine the direction of navigation for animations and history management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NavigationDirection {
    /// Navigating forward to a new route
    Forward,
    /// Navigating back in history
    Back,
    /// Replacing the current route without affecting history direction
    Replace,
}

/// Event emitted when the route changes.
///
/// Contains information about the navigation that occurred, including
/// the source and destination paths and the direction of navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteChangeEvent {
    /// The previous path (None if this is the first navigation)
    pub from: Option<String>,
    /// The new path being navigated to
    pub to: String,
    /// The direction of navigation
    pub direction: NavigationDirection,
}
