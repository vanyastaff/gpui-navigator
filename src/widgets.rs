//! RouterOutlet component for rendering nested routes
//!
//! The `RouterOutlet` acts as a placeholder where child routes are rendered.
//! When a parent route contains child routes, the outlet determines where
//! the matched child's content appears within the parent's layout.

use crate::context::GlobalRouter;
use crate::nested::resolve_child_route;
#[cfg(feature = "transition")]
use crate::transition::{SlideDirection, Transition};
use crate::{debug_log, error_log, trace_log, warn_log};
use gpui::{div, AnyElement, App, Div, IntoElement, ParentElement, SharedString, Styled, Window};

#[cfg(feature = "transition")]
use gpui::{relative, Animation, AnimationExt};

#[cfg(feature = "transition")]
use gpui::prelude::FluentBuilder;

#[cfg(feature = "transition")]
use std::time::Duration;

/// RouterOutlet component that renders the active child route
///
/// RouterOutlet is a special element that dynamically renders child routes
/// based on the current route match. It accesses the GlobalRouter to resolve
/// which child should be displayed.
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::{Route, RouterOutlet, RouteParams};
/// use gpui::*;
///
/// // Parent layout component
/// fn dashboard_layout(_cx: &mut App, _params: &RouteParams) -> AnyElement {
///     div()
///         .child("Dashboard Header")
///         .child(RouterOutlet::new()) // Child routes render here
///         .into_any_element()
/// }
///
/// // Configure nested routes
/// Route::new("/dashboard", dashboard_layout)
///     .children(vec![
///         Route::new("overview", |_, _cx, _params| div().into_any_element()),
///         Route::new("settings", |_, _cx, _params| div().into_any_element()),
///     ]);
/// ```
pub struct RouterOutlet {
    /// Optional name for named outlets
    /// Default outlet has no name
    name: Option<String>,
    /// Internal state for tracking animations and transitions
    /// Stored as RefCell to allow interior mutability during render
    state: std::cell::RefCell<OutletState>,
}

impl Clone for RouterOutlet {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            // Create a new state with cloned data from current state
            state: std::cell::RefCell::new(self.state.borrow().clone()),
        }
    }
}

impl RouterOutlet {
    /// Create a new default outlet
    pub fn new() -> Self {
        Self {
            name: None,
            state: std::cell::RefCell::new(OutletState::default()),
        }
    }

    /// Create a named outlet
    ///
    /// Named outlets allow multiple outlet locations in a single parent route.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_navigator::{RouterOutlet, RouteParams};
    /// use gpui::*;
    ///
    /// // Parent layout with multiple outlets
    /// fn app_layout(_cx: &mut App, _params: &RouteParams) -> AnyElement {
    ///     div()
    ///         .child(RouterOutlet::new()) // Main content
    ///         .child(RouterOutlet::named("sidebar")) // Sidebar content
    ///         .into_any_element()
    /// }
    /// ```
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            state: std::cell::RefCell::new(OutletState::default()),
        }
    }
}

impl Default for RouterOutlet {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a cached RouterOutlet that persists across renders
///
/// This is the recommended way to use RouterOutlet in your layouts.
/// It automatically caches the outlet entity to prevent state reset.
///
/// # Arguments
/// * `window` - The window context
/// * `cx` - The app context
/// * `key` - Unique key for this outlet (e.g., "main", "dashboard", "sidebar")
///
/// # Example
///
/// ```ignore
/// impl Render for MyLayout {
///     fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
///         div()
///             .child("My Layout")
///             .child(router_outlet(window, cx, "main"))
///     }
/// }
/// ```
pub fn router_outlet<V>(
    window: &mut gpui::Window,
    cx: &mut gpui::Context<'_, V>,
    key: impl Into<String>,
) -> impl gpui::IntoElement {
    window
        .use_keyed_state(gpui::ElementId::Name(key.into().into()), cx, |_, _| {
            RouterOutlet::new()
        })
        .clone()
}

/// Create a cached named RouterOutlet
///
/// Like `router_outlet()` but for named outlets (multiple outlets in one layout).
///
/// # Example
///
/// ```ignore
/// impl Render for MyLayout {
///     fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
///         div()
///             .child(router_outlet(window, cx, "main"))           // default outlet
///             .child(router_outlet_named(window, cx, "sb", "sidebar"))  // named outlet
///     }
/// }
/// ```
pub fn router_outlet_named<V>(
    window: &mut gpui::Window,
    cx: &mut gpui::Context<'_, V>,
    key: impl Into<String>,
    name: impl Into<String>,
) -> impl gpui::IntoElement {
    window
        .use_keyed_state(gpui::ElementId::Name(key.into().into()), cx, move |_, _| {
            RouterOutlet::named(name)
        })
        .clone()
}

use gpui::{Context, Render};

/// State for RouterOutlet animation tracking
#[derive(Clone)]
struct OutletState {
    current_path: String,
    animation_counter: u32,
    // Current route data (will become previous on next transition)
    current_params: crate::RouteParams,
    current_builder: Option<crate::route::RouteBuilder>,
    #[cfg(feature = "transition")]
    current_transition: crate::transition::Transition,
    // Previous route info for exit animation
    previous_route: Option<PreviousRoute>,
}

#[derive(Clone)]
struct PreviousRoute {
    path: String,
    params: crate::RouteParams,
    builder: Option<crate::route::RouteBuilder>,
}

impl Default for OutletState {
    fn default() -> Self {
        Self {
            current_path: String::new(),
            animation_counter: 0,
            current_params: crate::RouteParams::new(),
            current_builder: None,
            #[cfg(feature = "transition")]
            current_transition: crate::transition::Transition::None,
            previous_route: None,
        }
    }
}

impl Render for RouterOutlet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        #[cfg(debug_assertions)]
        {
            static RENDER_COUNT: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            let count = RENDER_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count % 10 == 0 {
                eprintln!("⚠️ RouterOutlet::render() called {} times", count);
            }
        }
        trace_log!("🔄 RouterOutlet::render() called");

        // OPTIMIZATION: Check if path changed before doing expensive route resolution
        let router = cx.try_global::<crate::context::GlobalRouter>();

        let Some(router) = router else {
            // No router - render nothing (like React Router)
            trace_log!("⚠️ RouterOutlet: No global router found");
            return div().into_any_element();
        };

        let current_path = router.current_path().to_string();

        // Check cached path first to avoid unnecessary work
        let prev_path = {
            let state = self.state.borrow();
            state.current_path.clone()
        };

        // If path hasn't changed, render cached content
        if current_path == prev_path && !prev_path.is_empty() {
            let cached_result = {
                let state = self.state.borrow();
                state.current_builder.clone()
            };

            if let Some(builder) = cached_result {
                let cached_params = {
                    let state = self.state.borrow();
                    state.current_params.clone()
                };

                // Render cached route without re-resolving
                return builder(window, cx, &cached_params);
            }
        }

        // Path changed or no cache - resolve child route
        let (current_path, child_route, child_params) = {
            // Find parent route that has children
            let parent_route = find_parent_route_for_path(router.state().routes(), &current_path);

            let Some(parent_route) = parent_route else {
                // No parent with children - render nothing (like React Router)
                // This happens when RouterOutlet is used in a route without children
                #[cfg(debug_assertions)]
                eprintln!(
                    "⚠️ RouterOutlet: No parent route with children found for path '{}'",
                    current_path
                );
                trace_log!(
                    "⚠️ RouterOutlet: No parent route with children found for path '{}'",
                    current_path
                );
                return div().into_any_element();
            };

            // Resolve which CHILD route should be rendered
            // T033: Add performance logging for resolution >1ms
            let route_params = crate::RouteParams::new();

            #[cfg(debug_assertions)]
            let start = std::time::Instant::now();

            let resolved = resolve_child_route(
                parent_route,
                &current_path,
                &route_params,
                self.name.as_deref(),
            );

            #[cfg(debug_assertions)]
            {
                let elapsed = start.elapsed();
                if elapsed.as_micros() > 1000 {
                    debug_log!(
                        "⚠️ Slow route resolution: {}µs ({}ms) for path '{}'",
                        elapsed.as_micros(),
                        elapsed.as_millis(),
                        current_path
                    );
                }
            }

            let Some((child_route, child_params)) = resolved else {
                // No matching child - render nothing (like React Router)
                // This typically means the route configuration doesn't match the current path
                trace_log!(
                    "⚠️ RouterOutlet: No child route matched for path '{}'",
                    current_path
                );
                return div().into_any_element();
            };

            (current_path, child_route, child_params)
        };

        // Get previous path and animation counter from internal state
        let (prev_path, animation_counter) = {
            let state = self.state.borrow();
            (state.current_path.clone(), state.animation_counter)
        };

        // Get child route info for rendering
        #[cfg(feature = "transition")]
        let (router_path, route_params, route_transition, builder_opt) = {
            let path = current_path.to_string();
            let transition = child_route.transition.default.clone();
            let builder = child_route.builder.clone();
            (path, child_params, transition, builder)
        };

        #[cfg(not(feature = "transition"))]
        let (router_path, route_params, builder_opt) = {
            let path = current_path.to_string();
            let builder = child_route.builder.clone();
            (path, child_params, builder)
        };

        // Check if path actually changed (not just first render)
        let path_changed = router_path != prev_path;

        // Update state if path changed
        #[cfg_attr(not(feature = "transition"), allow(unused_variables))]
        let animation_counter = if path_changed {
            #[cfg_attr(not(feature = "transition"), allow(unused_variables))]
            let is_initial = prev_path.is_empty();

            #[cfg(feature = "transition")]
            let new_counter = if is_initial {
                debug_log!("Initial route: '{}', no animation", router_path);
                animation_counter
            } else {
                let counter = animation_counter.wrapping_add(1);
                debug_log!(
                    "Route changed: '{}' -> '{}', transition={:?}, animation_counter={}",
                    prev_path,
                    router_path,
                    route_transition,
                    counter
                );
                counter
            };

            #[cfg(not(feature = "transition"))]
            let new_counter = {
                debug_log!(
                    "Route changed: '{}' -> '{}' (no transition)",
                    prev_path,
                    router_path
                );
                animation_counter
            };

            // Update state and save previous route for exit animation
            {
                let mut state = self.state.borrow_mut();
                // When path changes, replace previous_route with current route data
                // This way, the old previous_route (from a previous transition) is discarded
                // and we only keep the immediately previous route for the current transition
                if !is_initial {
                    state.previous_route = Some(PreviousRoute {
                        path: state.current_path.clone(),
                        params: state.current_params.clone(),
                        builder: state.current_builder.clone(),
                    });
                } else {
                    // Initial navigation - no previous route
                    state.previous_route = None;
                }
                // Update state with NEW route data
                state.current_path = router_path.clone();
                state.current_params = route_params.clone();
                state.current_builder = builder_opt.clone();
                #[cfg(feature = "transition")]
                {
                    state.current_transition = route_transition.clone();
                }
                state.animation_counter = new_counter;
            }

            new_counter
        } else {
            trace_log!("Route unchanged: '{}'", router_path);
            animation_counter
        };

        #[cfg(feature = "transition")]
        {
            // Determine animation duration based on transition type
            // Don't zero out duration on subsequent renders - let animation complete!
            let duration_ms = match &route_transition {
                Transition::Fade { duration_ms, .. } => *duration_ms,
                Transition::Slide { duration_ms, .. } => *duration_ms,
                Transition::None => 0,
            };

            debug_log!(
                "Rendering route '{}' with animation_counter={}, duration={}ms",
                router_path,
                animation_counter,
                duration_ms
            );

            // Get previous route info for exit animation
            // Show it if it exists and its path differs from current path
            // (if paths are same, no transition is needed)
            let previous_route = {
                let state = self.state.borrow();
                state
                    .previous_route
                    .as_ref()
                    .filter(|prev| prev.path != router_path)
                    .cloned()
            };

            debug_log!(
                "Previous route exists: {}, path: {:?}",
                previous_route.is_some(),
                previous_route.as_ref().map(|p| &p.path)
            );

            // Build OLD and NEW content ONCE before match to avoid multiple builder() calls per render
            let old_content_opt = previous_route.map(|prev| {
                if let Some(builder) = prev.builder.as_ref() {
                    builder(window, cx, &prev.params)
                } else {
                    not_found_page().into_any_element()
                }
            });

            let new_content = if let Some(builder) = builder_opt.as_ref() {
                builder(window, cx, &route_params)
            } else {
                not_found_page().into_any_element()
            };

            // Build container with both old (exiting) and new (entering) content
            // For SLIDE transitions, use a different approach
            match &route_transition {
                Transition::Slide { direction, .. } => {
                    // Create animated container that holds BOTH elements side-by-side
                    let animation_id = SharedString::from(format!(
                        "outlet_slide_{:?}_{}",
                        self.name, animation_counter
                    ));

                    match direction {
                        SlideDirection::Left | SlideDirection::Right => {
                            // Horizontal slide: use absolute positioning for proper side-by-side layout
                            let is_left = matches!(direction, SlideDirection::Left);

                            div()
                                .relative()
                                .w_full()
                                .h_full()
                                .overflow_hidden()
                                // Old content (exits to left for SlideLeft, or stays for SlideRight)
                                .when_some(old_content_opt, |container, old| {
                                    container.child(
                                        div()
                                            .absolute()
                                            .w_full()
                                            .h_full()
                                            .child(old)
                                            .left(relative(0.0)) // Starts at normal position
                                            .with_animation(
                                                animation_id.clone(),
                                                Animation::new(Duration::from_millis(duration_ms)),
                                                move |this, delta| {
                                                    let progress = delta.clamp(0.0, 1.0);
                                                    // Old content exits
                                                    let offset = if is_left {
                                                        -progress // SlideLeft: old goes left (-1.0)
                                                    } else {
                                                        progress // SlideRight: old goes right (+1.0)
                                                    };
                                                    this.left(relative(offset))
                                                },
                                            ),
                                    )
                                })
                                // New content (enters from right for SlideLeft, from left for SlideRight)
                                .child(
                                    div()
                                        .absolute()
                                        .w_full()
                                        .h_full()
                                        .child(new_content)
                                        .left(relative(if is_left { 1.0 } else { -1.0 })) // Starts off-screen
                                        .with_animation(
                                            animation_id.clone(),
                                            Animation::new(Duration::from_millis(duration_ms)),
                                            move |this, delta| {
                                                let progress = delta.clamp(0.0, 1.0);
                                                // New content enters
                                                let start = if is_left { 1.0 } else { -1.0 };
                                                let offset = start * (1.0 - progress);
                                                this.left(relative(offset))
                                            },
                                        ),
                                )
                                .into_any_element()
                        }
                        SlideDirection::Up | SlideDirection::Down => {
                            // Vertical slide: use absolute positioning for proper stacked layout
                            let is_up = matches!(direction, SlideDirection::Up);

                            div()
                                .relative()
                                .w_full()
                                .h_full()
                                .overflow_hidden()
                                // Old content (exits up for SlideUp, or down for SlideDown)
                                .when_some(old_content_opt, |container, old| {
                                    container.child(
                                        div()
                                            .absolute()
                                            .w_full()
                                            .h_full()
                                            .child(old)
                                            .top(relative(0.0)) // Starts at normal position
                                            .with_animation(
                                                animation_id.clone(),
                                                Animation::new(Duration::from_millis(duration_ms)),
                                                move |this, delta| {
                                                    let progress = delta.clamp(0.0, 1.0);
                                                    // Old content exits
                                                    let offset = if is_up {
                                                        -progress // SlideUp: old goes up (-1.0)
                                                    } else {
                                                        progress // SlideDown: old goes down (+1.0)
                                                    };
                                                    this.top(relative(offset))
                                                },
                                            ),
                                    )
                                })
                                // New content (enters from bottom for SlideUp, from top for SlideDown)
                                .child(
                                    div()
                                        .absolute()
                                        .w_full()
                                        .h_full()
                                        .child(new_content)
                                        .top(relative(if is_up { 1.0 } else { -1.0 })) // Starts off-screen
                                        .with_animation(
                                            animation_id.clone(),
                                            Animation::new(Duration::from_millis(duration_ms)),
                                            move |this, delta| {
                                                let progress = delta.clamp(0.0, 1.0);
                                                // New content enters
                                                let start = if is_up { 1.0 } else { -1.0 };
                                                let offset = start * (1.0 - progress);
                                                this.top(relative(offset))
                                            },
                                        ),
                                )
                                .into_any_element()
                        }
                    }
                }
                Transition::Fade { .. } => {
                    div()
                        .relative()
                        .w_full()
                        .h_full()
                        .overflow_hidden()
                        // Old content (fades out)
                        .when_some(old_content_opt, |container, old| {
                            container.child(
                                div()
                                    .absolute()
                                    .w_full()
                                    .h_full()
                                    .child(old)
                                    .with_animation(
                                        SharedString::from(format!(
                                            "outlet_fade_exit_{:?}_{}",
                                            self.name, animation_counter
                                        )),
                                        Animation::new(Duration::from_millis(duration_ms)),
                                        |this, delta| {
                                            let progress = delta.clamp(0.0, 1.0);
                                            this.opacity(1.0 - progress)
                                        },
                                    ),
                            )
                        })
                        // New content (fades in)
                        .child(
                            div()
                                .absolute()
                                .w_full()
                                .h_full()
                                .child(new_content)
                                .opacity(0.0)
                                .with_animation(
                                    SharedString::from(format!(
                                        "outlet_fade_enter_{:?}_{}",
                                        self.name, animation_counter
                                    )),
                                    Animation::new(Duration::from_millis(duration_ms)),
                                    |this, delta| {
                                        let progress = delta.clamp(0.0, 1.0);
                                        this.opacity(progress)
                                    },
                                ),
                        )
                        .into_any_element()
                }
                _ => {
                    // No transition or unsupported - just show new content
                    div()
                        .relative()
                        .w_full()
                        .h_full()
                        .child(new_content)
                        .into_any_element()
                }
            }
        }

        #[cfg(not(feature = "transition"))]
        {
            // No animation support - just build content directly
            let animation_id = SharedString::from(format!("outlet_{:?}", self.name));
            build_animated_route_content(
                cx,
                window,
                builder_opt.as_ref(),
                &route_params,
                animation_id,
                &(),
                0,
            )
        }
    }
}

/// RouterOutletElement - a function-based element that can access App context
///
/// This is the functional approach that actually works with route builders.
/// It returns a function that will be called with App context to render child routes.
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::{render_router_outlet, RouteParams};
/// use gpui::*;
///
/// fn layout(cx: &mut App, _params: &RouteParams) -> AnyElement {
///     div()
///         .child("Header")
///         .child(render_router_outlet(cx, None)) // Pass cx explicitly
///         .into_any_element()
/// }
/// ```
pub fn render_router_outlet(window: &mut Window, cx: &mut App, name: Option<&str>) -> AnyElement {
    trace_log!("render_router_outlet called with name: {:?}", name);

    // Access GlobalRouter
    let router = cx.try_global::<GlobalRouter>();

    let Some(router) = router else {
        error_log!("No global router found - call init_router() first");
        return div()
            .child("RouterOutlet: No global router found. Call init_router() first.")
            .into_any_element();
    };

    let current_path = router.current_path();
    trace_log!("Current path: '{}'", current_path);

    // Find the parent route that has children and matches the current path
    // This searches through the route tree to find the correct parent
    let parent_route = find_parent_route_for_path(router.state().routes(), current_path);

    let Some(parent_route) = parent_route else {
        warn_log!(
            "No parent route with children found for path '{}'",
            current_path
        );
        return div()
            .child(format!(
                "RouterOutlet: No parent route with children found for path '{}'",
                current_path
            ))
            .into_any_element();
    };

    trace_log!(
        "Found parent route: '{}' with {} children",
        parent_route.config.path,
        parent_route.get_children().len()
    );

    // Check if parent route has children
    if parent_route.get_children().is_empty() {
        return div()
            .child(format!(
                "RouterOutlet: Route '{}' has no child routes",
                parent_route.config.path
            ))
            .into_any_element();
    }

    // Resolve which child route should be rendered.
    // We pass the current parent params; the resolver returns (route, merged_params).
    let route_params = crate::RouteParams::new();

    let resolved = resolve_child_route(parent_route, current_path, &route_params, name);

    let Some((child_route, child_params)) = resolved else {
        warn_log!("No child route matched for path '{}'", current_path);
        return div()
            .child(format!(
                "RouterOutlet: No child route matched for path '{}'",
                current_path
            ))
            .into_any_element();
    };

    trace_log!("Matched child route: '{}'", child_route.config.path);

    // Render the child route
    if let Some(builder) = &child_route.builder {
        // Call the builder with window, cx and parameters
        builder(window, cx, &child_params)
    } else {
        div()
            .child(format!(
                "RouterOutlet: Child route '{}' has no builder",
                child_route.config.path
            ))
            .into_any_element()
    }
}

/// Find the deepest parent route that should render in this outlet
///
/// This function performs a depth-first search through the route tree to find
/// the most specific route that:
/// 1. Has children (can contain a RouterOutlet)
/// 2. Matches (or is a parent of) the current path
///
/// # Algorithm
///
/// Uses depth-first search to find the deepest matching parent route.
/// For path `/dashboard/analytics`:
/// - Searches routes for one matching `/dashboard` with children
/// - If that route's children also have children matching the path, prefers the deeper one
/// - Returns the most specific parent route
///
/// # Time Complexity
///
/// O(n) where n is the total number of routes in the tree.
/// Early exits when routes don't have children.
///
/// # Example
///
/// ```text
/// Routes:
///   /dashboard (has children) -> matches /dashboard/analytics
///     /analytics (no children)
///     /settings (no children)
///
/// For path "/dashboard/analytics":
///   Returns: /dashboard route (has children)
/// ```
fn find_parent_route_for_path<'a>(
    routes: &'a [std::sync::Arc<crate::route::Route>],
    current_path: &str,
) -> Option<&'a std::sync::Arc<crate::route::Route>> {
    find_parent_route_internal(routes, current_path, "")
}

fn find_parent_route_internal<'a>(
    routes: &'a [std::sync::Arc<crate::route::Route>],
    current_path: &str,
    accumulated_path: &str,
) -> Option<&'a std::sync::Arc<crate::route::Route>> {
    let current_normalized = current_path.trim_start_matches('/').trim_end_matches('/');

    for route in routes {
        // Early exit: skip routes without children (can't be parent routes)
        if route.get_children().is_empty() {
            continue;
        }

        let route_segment = route
            .config
            .path
            .trim_start_matches('/')
            .trim_end_matches('/');

        // Build full path for this route
        let full_route_path = if accumulated_path.is_empty() {
            if route_segment.is_empty() || route_segment == "/" {
                String::new()
            } else {
                route_segment.to_string()
            }
        } else if route_segment.is_empty() || route_segment == "/" {
            accumulated_path.to_string()
        } else {
            format!("{}/{}", accumulated_path, route_segment)
        };

        // Check if current path is under this route's subtree
        let is_under = if full_route_path.is_empty() {
            !current_normalized.is_empty()
        } else {
            current_normalized.starts_with(&full_route_path)
                && (current_normalized.len() == full_route_path.len()
                    || current_normalized[full_route_path.len()..].starts_with('/'))
        };

        if is_under {
            // Depth-first: check children first for a deeper matching parent
            if let Some(deeper) =
                find_parent_route_internal(route.get_children(), current_path, &full_route_path)
            {
                return Some(deeper);
            }

            // Check if current path exactly matches this route
            // This means we're navigating TO this route (not through it to a child)
            // Return this route as the parent that should render its children (e.g., index route)
            // Example: path="/dashboard", route="/dashboard" -> return to render index/overview
            if current_normalized == full_route_path {
                return Some(route);
            }

            // No deeper parent found - check if any direct child matches or contains the current path
            // This route is the parent if current path is under one of its children
            for child in route.get_children() {
                let child_segment = child
                    .config
                    .path
                    .trim_start_matches('/')
                    .trim_end_matches('/');

                // Skip empty child segments (index routes) - they don't affect parent matching
                if child_segment.is_empty() {
                    continue;
                }

                let child_full_path = if full_route_path.is_empty() {
                    child_segment.to_string()
                } else {
                    format!("{}/{}", full_route_path, child_segment)
                };

                // Check if current path matches this child exactly
                // (only if child has no children, otherwise recursion would have caught it)
                if current_normalized == child_full_path && child.get_children().is_empty() {
                    return Some(route);
                }

                // Check for parameter routes (e.g., :id, :workspaceId, etc.)
                if child_segment.starts_with(':') {
                    // This is a parameter route - check if the path structure matches
                    // For /products/:id, if current is /products/3, the structure matches
                    let expected_prefix = if full_route_path.is_empty() {
                        String::new()
                    } else {
                        format!("{}/", full_route_path)
                    };

                    // Get the part after the parent path
                    let remainder = if expected_prefix.is_empty() {
                        current_normalized
                    } else {
                        current_normalized
                            .strip_prefix(&expected_prefix)
                            .unwrap_or("")
                    };

                    // If remainder has exactly one segment (no more slashes) AND child has no children,
                    // this param route matches
                    if !remainder.is_empty()
                        && !remainder.contains('/')
                        && child.get_children().is_empty()
                    {
                        trace_log!(
                            "  parameter route '{}' matches path segment '{}'",
                            child_segment,
                            remainder
                        );
                        return Some(route);
                    }

                    // FIXED: If remainder has multiple segments AND this child has children,
                    // this could be a nested parameter route (e.g., workspace/:id/project/:id)
                    // Return this route as parent so nested outlets can render
                    if !remainder.is_empty()
                        && remainder.contains('/')
                        && !child.get_children().is_empty()
                    {
                        trace_log!(
                            "  parameter route '{}' with children matches nested path '{}'",
                            child_segment,
                            remainder
                        );
                        return Some(route);
                    }
                }
            }
        }
    }

    None
}

// ============================================================================
// RouterView - Main Router Component (renders current route)
// ============================================================================

/// RouterView component that renders the current matched route
///
/// Use this at the ROOT level of your app to render top-level routes.
/// For nested child routes within a parent, use `RouterOutlet` instead.
///
/// # Example
///
/// ```ignore
/// struct MyApp {
///     router_view: Entity<RouterView>,
/// }
///
/// impl Render for MyApp {
///     fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
///         div().child(self.router_view.clone())
///     }
/// }
/// ```
pub struct RouterView;

impl RouterView {
    pub fn new() -> Self {
        Self
    }
}

impl Render for RouterView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        eprintln!("🔄 RouterView::render() called");
        router_view(window, cx)
    }
}

/// Functional RouterView - renders the current matched route
///
/// Use this at the ROOT level of your app to render top-level routes.
/// For nested child routes within a parent, use `router_outlet()` instead.
///
/// # Example
///
/// ```ignore
/// impl Render for MyApp {
///     fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
///         div().child(router_view(window, cx))
///     }
/// }
/// ```
pub fn router_view<V>(window: &mut Window, cx: &mut Context<'_, V>) -> AnyElement {
    use std::ops::DerefMut;

    // Get route and params from router, then drop the borrow
    let (route, route_params, _current_path) = {
        let router = cx.try_global::<crate::context::GlobalRouter>();

        let Some(router) = router else {
            eprintln!("⚠️ router_view: No global router found");
            return div().into_any_element();
        };

        let current_path = router.current_path().to_string();

        // Get current route for rendering (top-level, not deeply nested)
        let route = router.state().current_route_for_rendering();
        let route_match = router.state().current_match_immutable();

        let Some(route) = route else {
            eprintln!("⚠️ router_view: No route found for path '{}'", current_path);
            return div()
                .child(format!("404: No route found for '{}'", current_path))
                .into_any_element();
        };

        let route_params = if let Some(m) = route_match {
            let mut params = crate::RouteParams::new();
            for (key, value) in m.params {
                params.set(key, value);
            }
            params
        } else {
            crate::RouteParams::new()
        };

        (route.clone(), route_params, current_path)
    }; // router borrow ends here

    // Call the route builder to render the component
    if let Some(element) = route.build(window, cx.deref_mut(), &route_params) {
        element
    } else {
        div()
            .child("Error: Route has no builder")
            .into_any_element()
    }
}

// ============================================================================
// RouterLink - Navigation Link Component
// ============================================================================
//
// Provides a clickable link that navigates to a route when clicked.
// Similar to:

use crate::Navigator;
use gpui::*;

/// A clickable link component for router navigation
///
/// # Example
///
/// ```ignore
/// use gpui_navigator::RouterLink;
///
/// RouterLink::new("/products")
///     .child("View Products")
///     .build(cx)
/// ```
pub struct RouterLink {
    /// Target route path
    path: SharedString,
    /// Optional custom styling when link is active
    active_class: Option<Box<dyn Fn(Div) -> Div>>,
    /// Child elements
    children: Vec<AnyElement>,
}

impl RouterLink {
    /// Create a new RouterLink to the specified path
    pub fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            active_class: None,
            children: Vec::new(),
        }
    }

    /// Add a child element
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Set custom styling for when this link is active (current route)
    pub fn active_class(mut self, style: impl Fn(Div) -> Div + 'static) -> Self {
        self.active_class = Some(Box::new(style));
        self
    }

    /// Build the link element with the given context
    pub fn build<V: 'static>(self, cx: &mut Context<'_, V>) -> Div {
        let path = self.path.clone();
        let current_path = Navigator::current_path(cx);
        let is_active = current_path == path.as_ref();

        let mut link = div().cursor_pointer().on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_view, _event, _window, cx| {
                Navigator::push(cx, path.to_string());
                cx.notify();
            }),
        );

        // Apply active styling if provided and link is active
        if is_active {
            if let Some(active_fn) = self.active_class {
                link = active_fn(link);
            }
        }

        // Add children
        for child in self.children {
            link = link.child(child);
        }

        link
    }
}

/// Helper function to create a simple text link
pub fn router_link<V: 'static>(
    cx: &mut Context<'_, V>,
    path: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Div {
    let path_str: SharedString = path.into();
    let label_str: SharedString = label.into();
    let current_path = Navigator::current_path(cx);
    let is_active = current_path == path_str.as_ref();

    div()
        .cursor_pointer()
        .text_color(if is_active {
            rgb(0x2196f3)
        } else {
            rgb(0x333333)
        })
        .hover(|this| this.text_color(rgb(0x2196f3)))
        .child(label_str)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_view, _event, _window, cx| {
                Navigator::push(cx, path_str.to_string());
                cx.notify();
            }),
        )
}

// ============================================================================
// Default Pages System
// ============================================================================

/// Configuration for default router pages (404, loading, error, etc.)
pub struct DefaultPages {
    /// Custom 404 not found page builder
    pub not_found: Option<Box<dyn Fn() -> AnyElement + Send + Sync>>,
    /// Custom loading page builder
    pub loading: Option<Box<dyn Fn() -> AnyElement + Send + Sync>>,
    /// Custom error page builder
    pub error: Option<Box<dyn Fn(&str) -> AnyElement + Send + Sync>>,
}

impl DefaultPages {
    /// Create new default pages configuration with built-in defaults
    pub fn new() -> Self {
        Self {
            not_found: None,
            loading: None,
            error: None,
        }
    }

    /// Set custom 404 not found page
    pub fn with_not_found<F>(mut self, builder: F) -> Self
    where
        F: Fn() -> AnyElement + Send + Sync + 'static,
    {
        self.not_found = Some(Box::new(builder));
        self
    }

    /// Set custom loading page
    pub fn with_loading<F>(mut self, builder: F) -> Self
    where
        F: Fn() -> AnyElement + Send + Sync + 'static,
    {
        self.loading = Some(Box::new(builder));
        self
    }

    /// Set custom error page
    pub fn with_error<F>(mut self, builder: F) -> Self
    where
        F: Fn(&str) -> AnyElement + Send + Sync + 'static,
    {
        self.error = Some(Box::new(builder));
        self
    }

    /// Render 404 not found page (custom or default)
    pub fn render_not_found(&self) -> AnyElement {
        if let Some(builder) = &self.not_found {
            builder()
        } else {
            default_not_found_page().into_any_element()
        }
    }

    /// Render loading page (custom or default)
    pub fn render_loading(&self) -> AnyElement {
        if let Some(builder) = &self.loading {
            builder()
        } else {
            default_loading_page().into_any_element()
        }
    }

    /// Render error page (custom or default)
    pub fn render_error(&self, message: &str) -> AnyElement {
        if let Some(builder) = &self.error {
            builder(message)
        } else {
            default_error_page(message).into_any_element()
        }
    }
}

impl Default for DefaultPages {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Built-in Default Pages
// ============================================================================

/// Default 404 Not Found page
fn not_found_page() -> impl IntoElement {
    // For now, use the static default
    // In the future, this could check a global DefaultPages config
    default_not_found_page()
}

/// Built-in minimalist 404 page
fn default_not_found_page() -> impl IntoElement {
    use gpui::{div, relative, rgb, ParentElement, Styled};

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .p_8()
        .gap_6()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(140.))
                .h(px(140.))
                .rounded(px(24.))
                .bg(rgb(0xf44336))
                .shadow_lg()
                .child(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(64.))
                        .child("404"),
                ),
        )
        .child(
            div()
                .text_3xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child("Page Not Found"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xcccccc))
                .text_center()
                .max_w(px(500.))
                .line_height(relative(1.6))
                .child("The page you're looking for doesn't exist or has been moved."),
        )
        .child(
            div()
                .mt_4()
                .p_6()
                .bg(rgb(0x252526))
                .rounded(px(12.))
                .border_1()
                .border_color(rgb(0x3e3e3e))
                .max_w(px(600.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xf44336))
                                .mb_2()
                                .child("What happened?"),
                        )
                        .child(not_found_item("•", "The route doesn't exist in the router"))
                        .child(not_found_item("•", "The URL might be mistyped"))
                        .child(not_found_item("•", "The page may have been removed")),
                ),
        )
}

fn not_found_item(bullet: &str, text: &str) -> impl IntoElement {
    use gpui::{div, rgb, ParentElement, Styled};

    div()
        .flex()
        .items_start()
        .gap_3()
        .child(
            div()
                .text_sm()
                .text_color(rgb(0xf44336))
                .child(bullet.to_string()),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0xcccccc))
                .line_height(relative(1.5))
                .child(text.to_string()),
        )
}

/// Built-in minimalist loading page
fn default_loading_page() -> impl IntoElement {
    use gpui::{div, rgb, ParentElement, Styled};

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(80.))
                .h(px(80.))
                .rounded(px(16.))
                .bg(rgb(0x2196f3))
                .shadow_lg()
                .child(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(36.))
                        .child("⏳"),
                ),
        )
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0xffffff))
                .child("Loading..."),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x888888))
                .child("Please wait"),
        )
}

/// Built-in minimalist error page
fn default_error_page(message: &str) -> impl IntoElement {
    use gpui::{div, relative, rgb, ParentElement, Styled};

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .p_8()
        .gap_6()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(120.))
                .h(px(120.))
                .rounded(px(20.))
                .bg(rgb(0xff9800))
                .shadow_lg()
                .child(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(48.))
                        .child("⚠️"),
                ),
        )
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child("Something Went Wrong"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xcccccc))
                .text_center()
                .max_w(px(500.))
                .line_height(relative(1.6))
                .child(message.to_string()),
        )
        .child(
            div()
                .mt_2()
                .px_6()
                .py_3()
                .bg(rgb(0x252526))
                .rounded(px(8.))
                .border_1()
                .border_color(rgb(0x3e3e3e))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x888888))
                        .child("Try refreshing the page or contact support"),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::{find_parent_route_for_path, RouterOutlet};
    use crate::route::Route;
    use gpui::{div, IntoElement, ParentElement};
    use std::sync::Arc;

    #[test]
    fn test_outlet_creation() {
        let outlet = RouterOutlet::default();
        assert!(outlet.name.is_none());

        let named = RouterOutlet::named("sidebar");
        assert_eq!(named.name.as_deref(), Some("sidebar"));
    }

    #[test]
    fn test_outlet_name() {
        let outlet = RouterOutlet::new();
        assert!(outlet.name.is_none());

        let named = RouterOutlet::named("main");
        assert_eq!(named.name, Some("main".to_string()));
    }

    // Helper to create a dummy builder
    fn dummy_builder(
        _window: &mut gpui::Window,
        _cx: &mut gpui::App,
        _params: &crate::RouteParams,
    ) -> gpui::AnyElement {
        div().child("test").into_any_element()
    }

    #[test]
    fn test_find_parent_route_simple() {
        // Create route tree:
        // /dashboard (has children)
        //   /overview
        //   /analytics
        let routes = vec![Arc::new(Route::new("/dashboard", dummy_builder).children(
            vec![
                Arc::new(Route::new("overview", dummy_builder)),
                Arc::new(Route::new("analytics", dummy_builder)),
            ],
        ))];

        // Should find dashboard for /dashboard/analytics
        let result = find_parent_route_for_path(&routes, "/dashboard/analytics");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "/dashboard");
    }

    #[test]
    fn test_find_parent_route_exact_match() {
        let routes = vec![Arc::new(
            Route::new("/dashboard", dummy_builder)
                .children(vec![Arc::new(Route::new("settings", dummy_builder))]),
        )];

        // Should find dashboard even when path is exactly /dashboard
        let result = find_parent_route_for_path(&routes, "/dashboard");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "/dashboard");
    }

    #[test]
    fn test_find_parent_route_no_children() {
        // Route without children
        let routes = vec![Arc::new(Route::new("/about", dummy_builder))];

        // Should return None (no parent with children)
        let result = find_parent_route_for_path(&routes, "/about");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_parent_route_nested_parents() {
        // Create deeply nested route tree:
        // /dashboard (has children)
        //   /settings (has children)
        //     /profile
        let routes = vec![Arc::new(Route::new("/dashboard", dummy_builder).children(
            vec![Arc::new(Route::new("settings", dummy_builder).children(
                vec![Arc::new(Route::new("profile", dummy_builder))],
            ))],
        ))];

        // Should find the deepest parent with children (settings)
        let result = find_parent_route_for_path(&routes, "/dashboard/settings/profile");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "settings");
    }

    #[test]
    fn test_find_parent_route_root() {
        // Root route with children
        let routes = vec![Arc::new(Route::new("/", dummy_builder).children(vec![
            Arc::new(Route::new("home", dummy_builder)),
            Arc::new(Route::new("about", dummy_builder)),
        ]))];

        // Root route should match any path
        let result = find_parent_route_for_path(&routes, "/home");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "/");

        let result = find_parent_route_for_path(&routes, "/about");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "/");
    }

    #[test]
    fn test_find_parent_route_multiple_top_level() {
        // Multiple top-level routes
        let routes = vec![
            Arc::new(
                Route::new("/dashboard", dummy_builder)
                    .children(vec![Arc::new(Route::new("overview", dummy_builder))]),
            ),
            Arc::new(
                Route::new("/settings", dummy_builder)
                    .children(vec![Arc::new(Route::new("profile", dummy_builder))]),
            ),
        ];

        // Should find correct parent
        let result = find_parent_route_for_path(&routes, "/dashboard/overview");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "/dashboard");

        let result = find_parent_route_for_path(&routes, "/settings/profile");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "/settings");
    }

    #[test]
    fn test_find_parent_route_no_match() {
        let routes = vec![Arc::new(
            Route::new("/dashboard", dummy_builder)
                .children(vec![Arc::new(Route::new("overview", dummy_builder))]),
        )];

        // Non-existent path
        let result = find_parent_route_for_path(&routes, "/nonexistent/path");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_parent_route_trailing_slash() {
        let routes = vec![Arc::new(
            Route::new("/dashboard/", dummy_builder)
                .children(vec![Arc::new(Route::new("settings", dummy_builder))]),
        )];

        // Should handle trailing slashes
        let result = find_parent_route_for_path(&routes, "/dashboard/settings");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_parent_route_empty_child_path() {
        // Parent with index route (empty path child)
        let routes = vec![Arc::new(Route::new("/dashboard", dummy_builder).children(
            vec![
                Arc::new(Route::new("", dummy_builder)), // Index route
                Arc::new(Route::new("settings", dummy_builder)),
            ],
        ))];

        // Should still find parent
        let result = find_parent_route_for_path(&routes, "/dashboard");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "/dashboard");
    }

    #[test]
    fn test_find_parent_prefers_deepest() {
        // Test that depth-first search prefers deeper parents
        // / (has children)
        //   /dashboard (has children)
        //     /settings (has children)
        //       /profile
        let routes = vec![Arc::new(Route::new("/", dummy_builder).children(vec![
            Arc::new(
                Route::new("dashboard", dummy_builder).children(vec![Arc::new(
                Route::new("settings", dummy_builder)
                    .children(vec![Arc::new(Route::new("profile", dummy_builder))]),
            )]),
            ),
        ]))];

        // For /dashboard/settings/profile, should find settings (deepest with children)
        let result = find_parent_route_for_path(&routes, "/dashboard/settings/profile");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "settings");

        // For /dashboard/settings, should find dashboard (deepest with children)
        let result = find_parent_route_for_path(&routes, "/dashboard/settings");
        assert!(result.is_some());
        assert_eq!(result.unwrap().config.path, "dashboard");
    }
}
