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
/// use gpui_router::{Route, RouterOutlet, RouteParams};
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
///         Route::new("overview", |_cx, _params| div().into_any_element()),
///         Route::new("settings", |_cx, _params| div().into_any_element()),
///     ]);
/// ```
#[derive(Clone)]
pub struct RouterOutlet {
    /// Optional name for named outlets
    /// Default outlet has no name
    name: Option<String>,
}

impl RouterOutlet {
    /// Create a new default outlet
    pub fn new() -> Self {
        Self { name: None }
    }

    /// Create a named outlet
    ///
    /// Named outlets allow multiple outlet locations in a single parent route.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui_router::{RouterOutlet, RouteParams};
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
        }
    }
}

impl Default for RouterOutlet {
    fn default() -> Self {
        Self::new()
    }
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
        trace_log!("🔄 RouterOutlet::render() called");

        // Use keyed state to persist animation counter and content across renders
        let state_key = SharedString::from(format!("outlet_{:?}", self.name));
        let state = window.use_keyed_state(state_key.clone(), cx, |_, _| OutletState::default());

        let (prev_path, animation_counter) = {
            let guard = state.read(cx);
            (guard.current_path.clone(), guard.animation_counter)
        };

        // Get current router info
        #[cfg(feature = "transition")]
        let (router_path, route_params, route_transition, builder_opt) = cx
            .try_global::<crate::context::GlobalRouter>()
            .map(|router| {
                let path = router.current_path().to_string();

                let params = router
                    .current_match_immutable()
                    .map(|m| {
                        let mut rp = crate::RouteParams::new();
                        for (k, v) in m.params {
                            rp.insert(k, v);
                        }
                        rp
                    })
                    .unwrap_or_else(crate::RouteParams::new);

                let transition = router
                    .current_route()
                    .map(|route| route.transition.default.clone())
                    .unwrap_or(Transition::None);

                let builder = router
                    .current_route()
                    .and_then(|route| route.builder.clone());

                (path, params, transition, builder)
            })
            .unwrap_or_else(|| {
                (
                    "/".to_string(),
                    crate::RouteParams::new(),
                    Transition::None,
                    None,
                )
            });

        #[cfg(not(feature = "transition"))]
        let (router_path, route_params, builder_opt) = cx
            .try_global::<crate::context::GlobalRouter>()
            .map(|router| {
                let path = router.current_path().to_string();

                let params = router
                    .current_match_immutable()
                    .map(|m| {
                        let mut rp = crate::RouteParams::new();
                        for (k, v) in m.params {
                            rp.insert(k, v);
                        }
                        rp
                    })
                    .unwrap_or_else(crate::RouteParams::new);

                let builder = router
                    .current_route()
                    .and_then(|route| route.builder.clone());

                (path, params, builder)
            })
            .unwrap_or_else(|| ("/".to_string(), crate::RouteParams::new(), None));

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
            state.update(cx, |s, _| {
                // When path changes, replace previous_route with current route data
                // This way, the old previous_route (from a previous transition) is discarded
                // and we only keep the immediately previous route for the current transition
                if !is_initial {
                    s.previous_route = Some(PreviousRoute {
                        path: s.current_path.clone(),
                        params: s.current_params.clone(),
                        builder: s.current_builder.clone(),
                    });
                } else {
                    // Initial navigation - no previous route
                    s.previous_route = None;
                }
                // Update state with NEW route data
                s.current_path = router_path.clone();
                s.current_params = route_params.clone();
                s.current_builder = builder_opt.clone();
                #[cfg(feature = "transition")]
                {
                    s.current_transition = route_transition.clone();
                }
                s.animation_counter = new_counter;
            });

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
            let previous_route = state
                .read(cx)
                .previous_route
                .as_ref()
                .filter(|prev| prev.path != router_path)
                .cloned();

            debug_log!(
                "Previous route exists: {}, path: {:?}",
                previous_route.is_some(),
                previous_route.as_ref().map(|p| &p.path)
            );

            // Build container with both old (exiting) and new (entering) content
            // For SLIDE transitions, use a different approach
            match &route_transition {
                Transition::Slide { direction, .. } => {
                    // Build OLD content (no animation wrapper, just the content)
                    let old_content = previous_route.map(|prev| {
                        if let Some(builder) = prev.builder.as_ref() {
                            builder(cx, &prev.params)
                        } else {
                            div().child("No route matched").into_any_element()
                        }
                    });

                    // Build NEW content (no animation wrapper, just the content)
                    let new_content = if let Some(builder) = builder_opt.as_ref() {
                        builder(cx, &route_params)
                    } else {
                        div().child("No route matched").into_any_element()
                    };

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
                                .when_some(old_content, |container, old| {
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
                                .when_some(old_content, |container, old| {
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
                    // Build OLD content (no animation wrapper, just the content)
                    let old_content = previous_route.map(|prev| {
                        if let Some(builder) = prev.builder.as_ref() {
                            builder(cx, &prev.params)
                        } else {
                            div().child("No route matched").into_any_element()
                        }
                    });

                    // Build NEW content (no animation wrapper, just the content)
                    let new_content = if let Some(builder) = builder_opt.as_ref() {
                        builder(cx, &route_params)
                    } else {
                        div().child("No route matched").into_any_element()
                    };

                    div()
                        .relative()
                        .w_full()
                        .h_full()
                        .overflow_hidden()
                        // Old content (fades out)
                        .when_some(old_content, |container, old| {
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
                    let new_content = if let Some(builder) = builder_opt.as_ref() {
                        builder(cx, &route_params)
                    } else {
                        div().child("No route matched").into_any_element()
                    };

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

/// Convenience function to create a default router outlet
///
/// # Example
///
/// ```ignore
/// use gpui_router::{router_outlet, RouteParams};
/// use gpui::*;
///
/// fn layout(cx: &mut App, _params: &RouteParams) -> AnyElement {
///     div()
///         .child("App Layout")
///         .child(router_outlet(cx)) // Child routes render here
///         .into_any_element()
/// }
/// ```
pub fn router_outlet(cx: &mut App) -> impl IntoElement {
    render_router_outlet(cx, None)
}

/// Convenience function to create a named router outlet
///
/// # Example
///
/// ```ignore
/// use gpui_router::{router_outlet, router_outlet_named, RouteParams};
/// use gpui::*;
///
/// fn layout(cx: &mut App, _params: &RouteParams) -> AnyElement {
///     div()
///         .child(router_outlet(cx)) // Main content
///         .child(router_outlet_named(cx, "sidebar")) // Sidebar
///         .into_any_element()
/// }
/// ```
pub fn router_outlet_named(cx: &mut App, name: impl Into<String>) -> impl IntoElement {
    render_router_outlet(cx, Some(&name.into()))
}

/// RouterOutletElement - a function-based element that can access App context
///
/// This is the functional approach that actually works with route builders.
/// It returns a function that will be called with App context to render child routes.
///
/// # Example
///
/// ```ignore
/// use gpui_router::{render_router_outlet, RouteParams};
/// use gpui::*;
///
/// fn layout(cx: &mut App, _params: &RouteParams) -> AnyElement {
///     div()
///         .child("Header")
///         .child(render_router_outlet(cx, None)) // Pass cx explicitly
///         .into_any_element()
/// }
/// ```
pub fn render_router_outlet(cx: &mut App, name: Option<&str>) -> AnyElement {
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
        // Call the builder with cx and parameters
        builder(cx, &child_params)
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

            // No deeper parent found - check if any direct child matches or contains the current path
            // This route is the parent if current path matches or is under one of its children
            for child in route.get_children() {
                let child_segment = child
                    .config
                    .path
                    .trim_start_matches('/')
                    .trim_end_matches('/');
                let child_full_path = if full_route_path.is_empty() {
                    child_segment.to_string()
                } else {
                    format!("{}/{}", full_route_path, child_segment)
                };

                // Check if current path matches this child or is under it
                if current_normalized == child_full_path
                    || current_normalized.starts_with(&format!("{}/", child_full_path))
                {
                    return Some(route);
                }
            }

            // If path exactly matches this route and no children matched,
            // return this route as parent (for rendering outlet when on the route itself)
            // Only do this if we're at the top level (accumulated_path is empty or this is the root)
            if current_normalized == full_route_path && accumulated_path.is_empty() {
                return Some(route);
            }
        }
    }

    None
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
/// use gpui_router::RouterLink;
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
            }),
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
    fn dummy_builder(_cx: &mut gpui::App, _params: &crate::RouteParams) -> gpui::AnyElement {
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
