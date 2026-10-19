//! Router widgets for rendering routes.
//!
//! This module provides the GPUI components that turn a matched route tree
//! into rendered UI:
//!
//! - [`RouterView`] / [`router_view`] — renders the **root** matched route
//!   (depth 0). Place this once in your top-level layout.
//! - [`RouterOutlet`] / [`router_outlet`] — renders the **child** route at
//!   the next nesting depth. Nest these inside route builders to compose
//!   parent-child layouts.
//! - [`RouterLink`] / [`router_link`] — clickable navigation link with
//!   optional active-state styling.
//! - [`DefaultPages`] — configurable fallback pages (404, loading, error).
//!
//! # Architecture (`MatchStack`)
//!
//! Instead of each outlet independently searching the route tree at render
//! time, [`GlobalRouter`] resolves a
//! [`MatchStack`](crate::resolve::MatchStack) **once per navigation**. Each
//! outlet reads its entry by depth index — **O(1) per outlet**.
//!
//! ```text
//! Navigation → resolve_match_stack() → [depth 0, depth 1, depth 2, …]
//!                                         ↑          ↑          ↑
//!                                     RouterView  Outlet#1   Outlet#2
//! ```

use crate::context::GlobalRouter;
use crate::resolve::{
    current_outlet_depth, enter_outlet, reset_outlet_depth, resolve_named_outlet, set_parent_depth,
};
use crate::{debug_log, trace_log};
#[allow(clippy::wildcard_imports)]
use gpui::*;

#[cfg(feature = "transition")]
use crate::transition::Transition;

#[cfg(feature = "transition")]
use gpui::{Animation, AnimationExt};

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
    /// Cached depth in the match stack. Computed once on first render via
    /// `enter_outlet()`, then reused on subsequent renders via `set_parent_depth()`.
    /// This avoids the thread-local `PARENT_DEPTH` growing stale between GPUI frames.
    depth: Option<usize>,
    /// Tracks the last rendered path for transition animations
    #[cfg(feature = "transition")]
    last_path: String,
    /// Animation counter for unique animation IDs
    #[cfg(feature = "transition")]
    animation_counter: u32,
    /// Transition currently being animated (persists across render frames)
    #[cfg(feature = "transition")]
    active_transition: Option<Transition>,
    /// When the current animation started
    #[cfg(feature = "transition")]
    transition_start: Option<std::time::Instant>,
}

impl Clone for RouterOutlet {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            depth: self.depth,
            #[cfg(feature = "transition")]
            last_path: self.last_path.clone(),
            #[cfg(feature = "transition")]
            animation_counter: self.animation_counter,
            #[cfg(feature = "transition")]
            active_transition: self.active_transition.clone(),
            #[cfg(feature = "transition")]
            transition_start: self.transition_start,
        }
    }
}

impl RouterOutlet {
    /// Create a new default outlet
    #[must_use]
    pub const fn new() -> Self {
        Self {
            name: None,
            depth: None,
            #[cfg(feature = "transition")]
            last_path: String::new(),
            #[cfg(feature = "transition")]
            animation_counter: 0,
            #[cfg(feature = "transition")]
            active_transition: None,
            #[cfg(feature = "transition")]
            transition_start: None,
        }
    }

    /// Create a named outlet
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            depth: None,
            #[cfg(feature = "transition")]
            last_path: String::new(),
            #[cfg(feature = "transition")]
            animation_counter: 0,
            #[cfg(feature = "transition")]
            active_transition: None,
            #[cfg(feature = "transition")]
            transition_start: None,
        }
    }
}

impl Default for RouterOutlet {
    fn default() -> Self {
        Self::new()
    }
}

impl RouterOutlet {
    /// Render a named outlet (separate from the enter/exit depth tracking).
    fn render_named(&self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let name = self.name.as_deref().unwrap_or("");

        // Try child cache first (mutable borrow, then drop)
        #[cfg(feature = "cache")]
        let cached = {
            let has_router = cx.try_global::<GlobalRouter>().is_some();
            if has_router {
                let current_path = cx.global::<GlobalRouter>().current_path().to_string();
                cx.update_global::<GlobalRouter, _>(|router, _| {
                    router
                        .nested_cache_mut()
                        .get_child(&current_path, Some(name))
                })
            } else {
                None
            }
        };

        let resolved = {
            let router = cx.try_global::<GlobalRouter>();

            let Some(router) = router else {
                trace_log!("RouterOutlet: No global router found");
                return div().into_any_element();
            };

            let current_path = router.current_path().to_string();
            let stack = router.match_stack();
            let depth = current_outlet_depth();

            let resolved = resolve_named_outlet(stack, depth, name, &current_path);
            if let Some((route, params)) = resolved {
                Some((route, params, current_path))
            } else {
                trace_log!("Named outlet '{}': no matching route", name);
                None
            }
        };

        let Some((route, params, current_path)) = resolved else {
            return div().into_any_element();
        };

        // Store in child cache on miss (after immutable borrow is released)
        #[cfg(feature = "cache")]
        if cached.is_none() {
            cx.update_global::<GlobalRouter, _>(|r, _| {
                r.nested_cache_mut().set_child(
                    current_path,
                    Some(name.to_string()),
                    params.clone(),
                );
            });
        }

        route.build(window, cx, &params).unwrap_or_else(|| {
            div()
                .child(format!("Route '{}' has no builder", route.config.path))
                .into_any_element()
        })
    }

    /// Apply transition animation, managing animation state across frames.
    ///
    /// Returns the element wrapped in a transition animation if a path change
    /// occurred or an animation is still in progress, otherwise returns the
    /// element with `last_path` updated.
    #[cfg(feature = "transition")]
    fn apply_transition(
        &mut self,
        element: AnyElement,
        transition: &Transition,
        current_path: String,
        my_depth: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let path_changed = current_path != self.last_path && !self.last_path.is_empty();

        if path_changed {
            self.animation_counter = self.animation_counter.wrapping_add(1);
            self.last_path = current_path;

            if !transition.is_none() {
                debug_log!(
                    "RouterOutlet depth {}: starting {:?} (counter={})",
                    my_depth,
                    transition,
                    self.animation_counter
                );
                self.active_transition = Some(transition.clone());
                self.transition_start = Some(std::time::Instant::now());
            }

            let exit_element = build_exit_element(my_depth, window, cx);
            return render_with_transition(
                element,
                exit_element,
                transition,
                self.name.as_ref(),
                self.animation_counter,
            );
        }

        // Animation still in progress — keep returning the wrapper
        if let (Some(active), Some(start)) = (&self.active_transition, self.transition_start) {
            if start.elapsed() < active.duration() {
                let exit_element = build_exit_element(my_depth, window, cx);
                return render_with_transition(
                    element,
                    exit_element,
                    active,
                    self.name.as_ref(),
                    self.animation_counter,
                );
            }
            // Animation finished — clear state
            self.active_transition = None;
            self.transition_start = None;
        }

        self.last_path = current_path;
        element
    }
}

/// Create a cached `RouterOutlet` that persists across renders
pub fn router_outlet<V>(
    window: &mut Window,
    cx: &mut Context<'_, V>,
    key: impl Into<String>,
) -> impl IntoElement {
    window.use_keyed_state(ElementId::Name(key.into().into()), cx, |_, _| {
        RouterOutlet::new()
    })
}

/// Create a cached named `RouterOutlet`
pub fn router_outlet_named<V>(
    window: &mut Window,
    cx: &mut Context<'_, V>,
    key: impl Into<String>,
    name: impl Into<String>,
) -> impl IntoElement {
    window.use_keyed_state(ElementId::Name(key.into().into()), cx, move |_, _| {
        RouterOutlet::named(name)
    })
}

impl Render for RouterOutlet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        // Named outlets don't use enter/exit — they resolve separately
        if self.name.is_some() {
            return self.render_named(window, cx);
        }

        // Default outlet: determine depth.
        //
        // First render: use enter_outlet() to discover depth from PARENT_DEPTH
        // thread-local and save it in self.depth for future renders.
        //
        // Subsequent renders: use saved depth and just set PARENT_DEPTH for
        // child outlets. This avoids the thread-local growing stale between
        // GPUI render frames (Entity components persist across frames).
        let my_depth = if let Some(d) = self.depth {
            // Already know our depth — just set PARENT_DEPTH for children
            set_parent_depth(d);
            d
        } else {
            // First render — discover depth from thread-local
            let d = enter_outlet();
            self.depth = Some(d);
            d
        };

        // Take the one-shot transition override before the immutable borrow.
        // Split into two statements to avoid overlapping borrows on `cx`.
        // Only touch the global when there is something to take: updating it
        // notifies every `observe_global::<GlobalRouter>` observer.
        #[cfg(feature = "transition")]
        let has_override = cx
            .try_global::<GlobalRouter>()
            .is_some_and(GlobalRouter::has_next_transition);
        #[cfg(feature = "transition")]
        let global_override: Option<Transition> = if has_override {
            cx.update_global::<GlobalRouter, _>(|router, _| router.take_next_transition())
        } else {
            None
        };

        // Extract data from router, then drop the borrow
        let resolved = {
            let router = cx.try_global::<GlobalRouter>();

            let Some(router) = router else {
                trace_log!("RouterOutlet: No global router found");
                return div().into_any_element();
            };

            let current_path = router.current_path().to_string();
            let stack = router.match_stack();

            let Some(entry) = stack.at_depth(my_depth) else {
                trace_log!(
                    "RouterOutlet depth {}: no entry in match stack (stack len={})",
                    my_depth,
                    stack.len()
                );
                return div().into_any_element();
            };

            debug_log!(
                "RouterOutlet depth {}: rendering route '{}' with {} params",
                my_depth,
                entry.route.config.path,
                entry.params.len()
            );

            // Priority: GlobalRouter override > TransitionConfig override_next > route default
            #[cfg(feature = "transition")]
            let transition = {
                let transition =
                    global_override.unwrap_or_else(|| entry.route.transition.active().clone());
                Some(if router.is_navigating_back() {
                    transition.reversed()
                } else {
                    transition
                })
            };
            #[cfg(not(feature = "transition"))]
            let transition = None::<()>;

            (
                std::sync::Arc::clone(&entry.route),
                entry.params.clone(),
                current_path,
                transition,
            )
        }; // router borrow ends here

        #[allow(clippy::used_underscore_binding)]
        let (route, params, current_path, _transition) = resolved;

        // Build the route component. PARENT_DEPTH is already set to Some(my_depth),
        // so any RouterOutlet rendered inside this builder (even deferred by GPUI)
        // will correctly get depth = my_depth + 1.
        let element = route.build(window, cx, &params).unwrap_or_else(|| {
            div()
                .child(format!("Route '{}' has no builder", route.config.path))
                .into_any_element()
        });

        // Apply transition animation if applicable
        #[cfg(feature = "transition")]
        if let Some(transition) = _transition {
            return self.apply_transition(element, &transition, current_path, my_depth, window, cx);
        }

        element
    }
}

/// Build exit content from the previous match stack (old route at same depth).
#[cfg(feature = "transition")]
fn build_exit_element(depth: usize, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let router = cx.try_global::<GlobalRouter>()?;
    let prev = router.previous_stack()?;
    let entry = prev.at_depth(depth)?;
    let route = std::sync::Arc::clone(&entry.route);
    let params = entry.params.clone();
    route.build(window, cx, &params)
}

/// Render content with a cross-transition animation (enter + exit).
///
/// When `exit_content` is provided, both old and new content are rendered
/// in a stacked container, each animated with the transition's style:
/// - **Fade**: old fades out (1→0), new fades in (0→1)
/// - **Slide Left**: old slides out left, new slides in from right
/// - **Slide Right**: old slides out right, new slides in from left
/// - **Slide Up/Down**: same pattern on the vertical axis
/// - **Custom**: the user's enter/exit styles
#[cfg(feature = "transition")]
fn render_with_transition(
    enter_content: AnyElement,
    exit_content: Option<AnyElement>,
    transition: &Transition,
    outlet_name: Option<&String>,
    counter: u32,
) -> AnyElement {
    let Some((enter_style, exit_style)) = transition.styles() else {
        return enter_content;
    };

    let mut animation = Animation::new(transition.duration());
    if let Some(easing) = transition.easing() {
        animation = animation.with_easing(easing);
    }
    let enter_id = SharedString::from(format!("outlet_enter_{outlet_name:?}_{counter}"));
    let exit_id = SharedString::from(format!("outlet_exit_{outlet_name:?}_{counter}"));
    let layer = || div().absolute().top_0().left_0().w_full().h_full();

    let mut container = div().relative().w_full().h_full().overflow_hidden();

    // Exit layer: old content animates out
    if let Some(exit) = exit_content {
        container = container.child(layer().child(exit).with_animation(
            exit_id,
            animation.clone(),
            move |this, delta| exit_style(this, delta.clamp(0.0, 1.0)),
        ));
    }

    // Enter layer: new content animates in (styled at progress 0 before the first frame)
    let initial = enter_style(layer().child(enter_content), 0.0);
    container
        .child(
            initial.with_animation(enter_id, animation, move |this, delta| {
                enter_style(this, delta.clamp(0.0, 1.0))
            }),
        )
        .into_any_element()
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
    // Named outlet: resolve separately (no enter/exit)
    if let Some(name) = name {
        // Try child cache first
        #[cfg(feature = "cache")]
        let cached = {
            let has_router = cx.try_global::<GlobalRouter>().is_some();
            if has_router {
                let current_path = cx.global::<GlobalRouter>().current_path().to_string();
                cx.update_global::<GlobalRouter, _>(|router, _| {
                    router
                        .nested_cache_mut()
                        .get_child(&current_path, Some(name))
                })
            } else {
                None
            }
        };

        let resolved = {
            let router = cx.try_global::<GlobalRouter>();

            let Some(router) = router else {
                return div().into_any_element();
            };

            let current_path = router.current_path().to_string();
            let stack = router.match_stack();
            let depth = current_outlet_depth();

            if let Some((route, params)) = resolve_named_outlet(stack, depth, name, &current_path) {
                Some((route, params, current_path))
            } else {
                trace_log!("render_router_outlet: named outlet '{}' not found", name);
                None
            }
        };

        let Some((route, params, current_path)) = resolved else {
            return div().into_any_element();
        };

        // Store in child cache on miss (after immutable borrow is released)
        #[cfg(feature = "cache")]
        if cached.is_none() {
            cx.update_global::<GlobalRouter, _>(|r, _| {
                r.nested_cache_mut().set_child(
                    current_path,
                    Some(name.to_string()),
                    params.clone(),
                );
            });
        }

        return route
            .build(window, cx, &params)
            .unwrap_or_else(|| div().into_any_element());
    }

    // Default outlet: PARENT_DEPTH determines depth automatically
    let my_depth = enter_outlet();

    let resolved = {
        let router = cx.try_global::<GlobalRouter>();

        let Some(router) = router else {
            return div().into_any_element();
        };

        let stack = router.match_stack();

        let Some(entry) = stack.at_depth(my_depth) else {
            trace_log!(
                "render_router_outlet: no entry at depth {} (stack len={})",
                my_depth,
                stack.len()
            );
            return div().into_any_element();
        };

        (std::sync::Arc::clone(&entry.route), entry.params.clone())
    }; // router borrow ends here

    let (route, params) = resolved;

    route
        .build(window, cx, &params)
        .unwrap_or_else(|| div().into_any_element())
}

// ============================================================================
// RouterView — top-level route renderer
// ============================================================================

/// Component that renders the root-level matched route (depth 0).
///
/// Use this as the top-level entry point in your window's `Render` impl.
/// Child routes are rendered by [`RouterOutlet`] instances nested inside
/// route builders.
pub struct RouterView;

impl Default for RouterView {
    fn default() -> Self {
        Self
    }
}

impl RouterView {
    /// Create a new `RouterView`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Render for RouterView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        router_view(window, cx)
    }
}

/// Functional `RouterView` — renders the top-level matched route (depth 0).
///
/// Resets outlet tracking to "no parent" and then calls `enter_outlet()` to
/// render `match_stack[0]`. Child outlets inside the builder will see
/// `PARENT_DEPTH = Some(0)` and render at depth 1, 2, 3...
pub fn router_view<V>(window: &mut Window, cx: &mut Context<'_, V>) -> AnyElement {
    // Reset to "no parent" — ensures router_view always starts as root
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
            // Try custom not-found handler first, fall back to built-in page
            if let Some(element) = router.error_handlers().render_not_found(cx, &current_path) {
                return element;
            }
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

    // enter_outlet: PARENT_DEPTH=None → depth=0, sets PARENT_DEPTH=Some(0)
    let _my_depth = enter_outlet();

    route
        .build(window, cx, &params)
        .unwrap_or_else(|| div().child("Root route has no builder").into_any_element())
}

// ============================================================================
// RouterLink
// ============================================================================

use crate::Navigator;

/// A clickable link component that navigates to a route on click.
///
/// Supports optional active-state styling via [`active_class`](Self::active_class).
///
/// # Examples
///
/// ```ignore
/// RouterLink::new("/settings")
///     .child("Settings")
///     .active_class(|div| div.text_color(gpui::rgb(0x21_96_f3)))
///     .build(cx)
/// ```
#[must_use]
pub struct RouterLink {
    /// Target route path
    path: SharedString,
    /// Optional custom styling when link is active
    active_class: Option<Box<dyn Fn(Div) -> Div>>,
    /// How the current path is compared with `path`
    active_match: ActiveMatch,
    /// Child elements
    children: Vec<AnyElement>,
}

/// How a [`RouterLink`] decides that it is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ActiveMatch {
    /// Active only on exactly the link's path (query string and trailing slash ignored).
    #[default]
    Exact,
    /// Also active on any path below the link's path, e.g. `/settings`
    /// is active on `/settings/profile`. Useful for sidebar sections.
    Prefix,
}

/// Return `true` if a link to `target` counts as active at `current`.
///
/// Query strings, fragments and leading/trailing slashes are ignored.
///
/// ```
/// use gpui_navigator::{is_link_active, ActiveMatch};
///
/// assert!(is_link_active("/users?page=2", "/users", ActiveMatch::Exact));
/// assert!(is_link_active("/settings/profile", "/settings", ActiveMatch::Prefix));
/// assert!(!is_link_active("/settingsx", "/settings", ActiveMatch::Prefix));
/// ```
#[must_use]
pub fn is_link_active(current: &str, target: &str, mode: ActiveMatch) -> bool {
    let current = crate::matcher::path_only(current).trim_matches('/');
    let target = crate::matcher::path_only(target).trim_matches('/');
    match mode {
        ActiveMatch::Exact => current == target,
        ActiveMatch::Prefix => {
            target.is_empty()
                || current == target
                || current
                    .strip_prefix(target)
                    .is_some_and(|rest| rest.starts_with('/'))
        }
    }
}

impl RouterLink {
    /// Create a new `RouterLink` to the specified path
    pub fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            active_class: None,
            active_match: ActiveMatch::Exact,
            children: Vec::new(),
        }
    }

    /// Choose how the link decides it is active (default [`ActiveMatch::Exact`]).
    pub const fn active_match(mut self, mode: ActiveMatch) -> Self {
        self.active_match = mode;
        self
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
        let is_active = is_link_active(&current_path, &path, self.active_match);

        // Navigate on release, like a button: a press that is dragged away
        // and released elsewhere does not navigate.
        let mut link = div().cursor_pointer().on_mouse_up(
            MouseButton::Left,
            cx.listener(move |_view, _event, _window, cx| {
                Navigator::push(cx, path.to_string());
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

/// Create a simple text link with built-in active-state color.
///
/// For more control (custom children, styling), use [`RouterLink`] directly.
pub fn router_link<V: 'static>(
    cx: &mut Context<'_, V>,
    path: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Div {
    let path_str: SharedString = path.into();
    let label_str: SharedString = label.into();
    let current_path = Navigator::current_path(cx);
    let is_active = is_link_active(&current_path, &path_str, ActiveMatch::Exact);

    div()
        .cursor_pointer()
        .text_color(if is_active {
            rgb(0x21_96_f3)
        } else {
            rgb(0x33_33_33)
        })
        .hover(|this| this.text_color(rgb(0x21_96_f3)))
        .child(label_str)
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |_view, _event, _window, cx| {
                Navigator::push(cx, path_str.to_string());
            }),
        )
}

// ============================================================================
// Default Pages System
// ============================================================================

/// Configurable fallback pages for 404, loading, and error states.
///
/// Register custom renderers or fall back to the built-in minimalist pages.
///
/// # Examples
///
/// ```ignore
/// DefaultPages::new()
///     .with_not_found(|| gpui::div().child("Custom 404").into_any_element())
///     .with_error(|msg| gpui::div().child(msg.to_string()).into_any_element())
/// ```
#[must_use]
pub struct DefaultPages {
    /// Custom 404 not found page builder
    pub not_found: Option<Box<dyn Fn() -> AnyElement + Send + Sync>>,
    /// Custom loading page builder
    pub loading: Option<Box<dyn Fn() -> AnyElement + Send + Sync>>,
    /// Custom error page builder
    #[allow(clippy::type_complexity)]
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
    #[must_use]
    pub fn render_not_found(&self) -> AnyElement {
        self.not_found
            .as_ref()
            .map_or_else(|| default_not_found_page("").into_any_element(), |b| b())
    }

    /// Render loading page (custom or default)
    #[must_use]
    pub fn render_loading(&self) -> AnyElement {
        self.loading
            .as_ref()
            .map_or_else(|| default_loading_page().into_any_element(), |b| b())
    }

    /// Render error page (custom or default)
    #[must_use]
    pub fn render_error(&self, message: &str) -> AnyElement {
        self.error.as_ref().map_or_else(
            || default_error_page(message).into_any_element(),
            |b| b(message),
        )
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
        .bg(rgb(0x1e_1e_1e))
        .p_8()
        .gap_6()
        .child(
            div()
                .text_3xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xff_ff_ff))
                .child("404 — Page Not Found"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xcc_cc_cc))
                .child(format!("No route matches: {path}")),
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
        .bg(rgb(0x1e_1e_1e))
        .gap_4()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0xff_ff_ff))
                .child("Loading..."),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x88_88_88))
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
        .bg(rgb(0x1e_1e_1e))
        .p_8()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xff_ff_ff))
                .child("Something Went Wrong"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xcc_cc_cc))
                .text_center()
                .max_w(px(500.))
                .line_height(relative(1.6))
                .child(message.to_string()),
        )
}

// ============================================================================
// Debug panel
// ============================================================================

/// A small read-only panel showing the router's state, for development.
///
/// Lists the current location, the matched route at every depth with its
/// parameters, the history with the cursor marked, any navigation waiting on
/// async guards, and the component cache size. Drop it anywhere in a view:
///
/// ```no_run
/// use gpui::*;
/// use gpui_navigator::{router_debug_panel, router_view};
///
/// struct AppShell;
///
/// impl Render for AppShell {
///     fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
///         div()
///             .size_full()
///             .child(router_view(window, cx))
///             .when(cfg!(debug_assertions), |this| this.child(router_debug_panel(cx)))
///     }
/// }
/// ```
pub fn router_debug_panel(cx: &App) -> Div {
    let panel = div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .text_xs()
        .font_family("monospace")
        .bg(rgba(0x0000_00cc))
        .text_color(rgb(0x00e0_e0e0));

    let Some(router) = cx.try_global::<GlobalRouter>() else {
        return panel.child("router: not initialized");
    };

    let mut panel = panel.child(format!("location: {}", router.current_path()));
    if let Some(pending) = router.pending_path() {
        panel = panel.child(format!("pending: {pending} (async guards)"));
    }

    panel = panel.child("match stack:");
    let stack = router.match_stack();
    if stack.is_empty() {
        panel = panel.child("  (no match → not found page)");
    }
    for entry in stack.entries() {
        let mut params: Vec<_> = entry
            .params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        params.sort();
        let params = if params.is_empty() {
            String::new()
        } else {
            format!("  {{{}}}", params.join(", "))
        };
        panel = panel.child(format!(
            "  [{}] {}{}",
            entry.depth,
            if entry.route.config.path.is_empty() {
                "(index)"
            } else {
                &entry.route.config.path
            },
            params
        ));
    }

    let snapshot = router.history_snapshot();
    panel = panel.child(format!("history ({}):", snapshot.entries.len()));
    for (i, entry) in snapshot.entries.iter().enumerate() {
        let marker = if i == snapshot.current { "▶" } else { " " };
        panel = panel.child(format!(" {marker} {}", entry.path));
    }

    panel.child(format!("cached pages: {}", router.component_cache_len()))
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
