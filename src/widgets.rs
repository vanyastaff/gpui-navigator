//! Router widgets for rendering routes
//!
//! Provides `RouterOutlet`, `RouterView`, `RouterLink` and helper functions
//! for rendering matched routes using the MatchStack architecture.
//!
//! ## Architecture (MatchStack)
//!
//! Instead of each outlet independently searching the route tree at render time,
//! `GlobalRouter` resolves a `MatchStack` once per navigation. Each outlet
//! reads its entry by depth index — O(1) per outlet.

use crate::context::GlobalRouter;
use crate::resolve::{
    claim_outlet_depth, current_outlet_depth, reset_outlet_depth, resolve_named_outlet,
    set_outlet_depth,
};
use crate::{debug_log, trace_log};
use gpui::*;

#[cfg(feature = "transition")]
use crate::transition::{SlideDirection, Transition};

#[cfg(feature = "transition")]
use gpui::{Animation, AnimationExt};

#[cfg(feature = "transition")]
use std::time::Duration;

// ============================================================================
// RouterOutlet (MatchStack-based — no RefCell)
// ============================================================================

/// Outlet component that renders the matched child route at this nesting depth.
///
/// # How it works
///
/// 1. On navigation, `GlobalRouter` resolves a `MatchStack` — an ordered list
///    of matched routes from root to leaf.
/// 2. `RouterView` resets the depth counter to 0 and renders `match_stack[0]`.
/// 3. Each `RouterOutlet` claims the next depth and renders `match_stack[depth]`.
///
/// This is O(1) per outlet instead of the previous O(n) tree search.
pub struct RouterOutlet {
    /// Optional outlet name (for named outlets like "sidebar")
    name: Option<String>,
    /// Tracks the last rendered path for transition animations
    #[cfg(feature = "transition")]
    last_path: String,
    /// Animation counter for unique animation IDs
    #[cfg(feature = "transition")]
    animation_counter: u32,
}

impl Clone for RouterOutlet {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            #[cfg(feature = "transition")]
            last_path: self.last_path.clone(),
            #[cfg(feature = "transition")]
            animation_counter: self.animation_counter,
        }
    }
}

impl RouterOutlet {
    /// Create a new default outlet
    pub fn new() -> Self {
        Self {
            name: None,
            #[cfg(feature = "transition")]
            last_path: String::new(),
            #[cfg(feature = "transition")]
            animation_counter: 0,
        }
    }

    /// Create a named outlet
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            #[cfg(feature = "transition")]
            last_path: String::new(),
            #[cfg(feature = "transition")]
            animation_counter: 0,
        }
    }
}

impl Default for RouterOutlet {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a cached RouterOutlet that persists across renders
pub fn router_outlet<V>(
    window: &mut Window,
    cx: &mut Context<'_, V>,
    key: impl Into<String>,
) -> impl IntoElement {
    window
        .use_keyed_state(ElementId::Name(key.into().into()), cx, |_, _| {
            RouterOutlet::new()
        })
        .clone()
}

/// Create a cached named RouterOutlet
pub fn router_outlet_named<V>(
    window: &mut Window,
    cx: &mut Context<'_, V>,
    key: impl Into<String>,
    name: impl Into<String>,
) -> impl IntoElement {
    window
        .use_keyed_state(ElementId::Name(key.into().into()), cx, move |_, _| {
            RouterOutlet::named(name)
        })
        .clone()
}

impl Render for RouterOutlet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        // Extract all data from router, then drop the borrow
        let resolved = {
            let router = cx.try_global::<GlobalRouter>();

            let Some(router) = router else {
                trace_log!("RouterOutlet: No global router found");
                return div().into_any_element();
            };

            let current_path = router.current_path().to_string();
            let stack = router.match_stack();

            // Named outlet: special resolution
            if let Some(ref name) = self.name {
                let depth = current_outlet_depth();
                let resolved = resolve_named_outlet(stack, depth, name, &current_path);
                if let Some((route, params)) = resolved {
                    Some((route, params, current_path, None))
                } else {
                    trace_log!("Named outlet '{}': no matching route", name);
                    return div().into_any_element();
                }
            } else {
                // Default outlet: claim next depth from match stack
                let depth = claim_outlet_depth();

                let Some(entry) = stack.at_depth(depth) else {
                    trace_log!(
                        "RouterOutlet depth {}: no entry in match stack (stack len={})",
                        depth,
                        stack.len()
                    );
                    return div().into_any_element();
                };

                debug_log!(
                    "RouterOutlet depth {}: rendering route '{}' with {} params",
                    depth,
                    entry.route.config.path,
                    entry.params.len()
                );

                #[cfg(feature = "transition")]
                let transition = Some(entry.route.transition.default.clone());
                #[cfg(not(feature = "transition"))]
                let transition = None::<()>;

                Some((
                    std::sync::Arc::clone(&entry.route),
                    entry.params.clone(),
                    current_path,
                    transition,
                ))
            }
        }; // router borrow ends here

        #[allow(clippy::used_underscore_binding)]
        let Some((route, params, current_path, _transition)) = resolved
        else {
            return div().into_any_element();
        };

        // Save/restore depth for nested outlets
        let saved_depth = current_outlet_depth();
        if self.name.is_none() {
            // For default outlets, set depth so nested outlets get depth+1
            let depth = saved_depth; // claim already incremented
            set_outlet_depth(depth.saturating_sub(1));
        }

        let element = route.build(window, cx, &params).unwrap_or_else(|| {
            div()
                .child(format!("Route '{}' has no builder", route.config.path))
                .into_any_element()
        });

        // Restore depth for sibling outlets
        set_outlet_depth(saved_depth);

        // Apply transition animation if path changed
        #[cfg(feature = "transition")]
        {
            if let Some(transition) = _transition {
                let path_changed = current_path != self.last_path && !self.last_path.is_empty();

                if path_changed {
                    self.animation_counter = self.animation_counter.wrapping_add(1);
                    self.last_path = current_path;

                    return render_with_transition(
                        element,
                        &transition,
                        self.name.as_ref(),
                        self.animation_counter,
                    );
                }

                self.last_path = current_path;
            }
        }

        element
    }
}

/// Render content with a transition animation (enter only, no exit animation in simplified version)
#[cfg(feature = "transition")]
fn render_with_transition(
    content: AnyElement,
    transition: &Transition,
    outlet_name: Option<&String>,
    counter: u32,
) -> AnyElement {
    match transition {
        Transition::Fade { duration_ms, .. } => {
            let duration = *duration_ms;
            div()
                .relative()
                .w_full()
                .h_full()
                .child(
                    div()
                        .w_full()
                        .h_full()
                        .child(content)
                        .opacity(0.0)
                        .with_animation(
                            SharedString::from(format!(
                                "outlet_fade_{:?}_{}",
                                outlet_name, counter
                            )),
                            Animation::new(Duration::from_millis(duration)),
                            |this, delta| {
                                let progress = delta.clamp(0.0, 1.0);
                                this.opacity(progress)
                            },
                        ),
                )
                .into_any_element()
        }
        Transition::Slide {
            duration_ms,
            direction,
            ..
        } => {
            let duration = *duration_ms;
            let animation_id =
                SharedString::from(format!("outlet_slide_{:?}_{}", outlet_name, counter));

            match direction {
                SlideDirection::Left | SlideDirection::Right => {
                    let is_left = matches!(direction, SlideDirection::Left);
                    div()
                        .relative()
                        .w_full()
                        .h_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .w_full()
                                .h_full()
                                .child(content)
                                .left(relative(if is_left { 1.0 } else { -1.0 }))
                                .with_animation(
                                    animation_id,
                                    Animation::new(Duration::from_millis(duration)),
                                    move |this, delta| {
                                        let progress = delta.clamp(0.0, 1.0);
                                        let start = if is_left { 1.0 } else { -1.0 };
                                        let offset = start * (1.0 - progress);
                                        this.left(relative(offset))
                                    },
                                ),
                        )
                        .into_any_element()
                }
                SlideDirection::Up | SlideDirection::Down => {
                    let is_up = matches!(direction, SlideDirection::Up);
                    div()
                        .relative()
                        .w_full()
                        .h_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .w_full()
                                .h_full()
                                .child(content)
                                .top(relative(if is_up { 1.0 } else { -1.0 }))
                                .with_animation(
                                    animation_id,
                                    Animation::new(Duration::from_millis(duration)),
                                    move |this, delta| {
                                        let progress = delta.clamp(0.0, 1.0);
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
        Transition::None => content,
    }
}

// ============================================================================
// render_router_outlet — Functional API
// ============================================================================

/// Render the child route at the next nesting depth.
///
/// This is the functional equivalent of `RouterOutlet`. Use it inside
/// route builders to render child content.
///
/// # Arguments
///
/// - `name`: `None` for default outlet, `Some("sidebar")` for named outlet
pub fn render_router_outlet(window: &mut Window, cx: &mut App, name: Option<&str>) -> AnyElement {
    // Extract data from router, then drop the borrow before calling build
    let resolved = {
        let router = cx.try_global::<GlobalRouter>();

        let Some(router) = router else {
            return div().into_any_element();
        };

        let current_path = router.current_path().to_string();
        let stack = router.match_stack();

        // Named outlet
        if let Some(name) = name {
            let depth = current_outlet_depth();

            if let Some((route, params)) = resolve_named_outlet(stack, depth, name, &current_path) {
                Some((route, params))
            } else {
                trace_log!("render_router_outlet: named outlet '{}' not found", name);
                return div().into_any_element();
            }
        } else {
            // Default outlet: claim next depth
            let depth = claim_outlet_depth();

            let Some(entry) = stack.at_depth(depth) else {
                trace_log!(
                    "render_router_outlet: no entry at depth {} (stack len={})",
                    depth,
                    stack.len()
                );
                return div().into_any_element();
            };

            Some((std::sync::Arc::clone(&entry.route), entry.params.clone()))
        }
    }; // router borrow ends here

    let Some((route, params)) = resolved else {
        return div().into_any_element();
    };

    // Set depth so nested outlets in the builder get depth+1
    let saved = current_outlet_depth();
    let current = saved.saturating_sub(1);
    set_outlet_depth(current);

    let element = route
        .build(window, cx, &params)
        .unwrap_or_else(|| div().into_any_element());

    set_outlet_depth(saved);
    element
}

// ============================================================================
// RouterView — top-level route renderer
// ============================================================================

/// RouterView component that renders the current matched route
pub struct RouterView;

impl Default for RouterView {
    fn default() -> Self {
        Self
    }
}

impl RouterView {
    pub fn new() -> Self {
        Self
    }
}

impl Render for RouterView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        router_view(window, cx)
    }
}

/// Functional RouterView — renders the top-level matched route (depth 0).
///
/// Resets the outlet depth counter and renders the first route in the match stack.
pub fn router_view<V>(window: &mut Window, cx: &mut Context<'_, V>) -> AnyElement {
    // Reset depth counter — this is the root of the render tree
    reset_outlet_depth();

    // Extract data from router, then drop borrow
    let resolved = {
        let router = cx.try_global::<GlobalRouter>();

        let Some(router) = router else {
            return div().child("No router configured").into_any_element();
        };

        let stack = router.match_stack();

        let Some(root_entry) = stack.root() else {
            let current_path = router.current_path().to_string();
            return default_not_found_page(&current_path).into_any_element();
        };

        debug_log!(
            "router_view: rendering root route '{}', stack depth={}",
            root_entry.route.config.path,
            stack.len()
        );

        (
            std::sync::Arc::clone(&root_entry.route),
            root_entry.params.clone(),
        )
    }; // router borrow ends here

    let (route, params) = resolved;

    // Depth is 0 — nested outlets will claim 1, 2, 3...
    set_outlet_depth(0);

    route
        .build(window, cx, &params)
        .unwrap_or_else(|| div().child("Root route has no builder").into_any_element())
}

// ============================================================================
// RouterLink
// ============================================================================

use crate::Navigator;

/// A clickable link component for router navigation
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

        if is_active {
            if let Some(active_fn) = self.active_class {
                link = active_fn(link);
            }
        }

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
            default_not_found_page("").into_any_element()
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

/// Built-in minimalist 404 page
fn default_not_found_page(path: &str) -> impl IntoElement {
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
                .text_3xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child("404 — Page Not Found"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xcccccc))
                .child(format!("No route matches: {}", path)),
        )
}

/// Built-in minimalist loading page
fn default_loading_page() -> impl IntoElement {
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
}

#[cfg(test)]
mod tests {
    use super::RouterOutlet;

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
}
