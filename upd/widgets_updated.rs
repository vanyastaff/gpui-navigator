//! Router widgets (UPDATED for MatchStack)
//!
//! Major simplifications:
//! - RouterOutlet no longer needs RefCell or OutletState
//! - render_router_outlet is ~30 lines instead of ~200
//! - find_parent_route_for_path DELETED (140 lines)
//! - No more recursive route resolution at render time

use crate::context::GlobalRouter;
use crate::resolve::{
    claim_outlet_depth, current_outlet_depth, reset_outlet_depth,
    resolve_named_outlet, set_outlet_depth,
};
use crate::{debug_log, trace_log};
use gpui::*;
use std::ops::DerefMut;

// ============================================================================
// RouterOutlet (SIMPLIFIED — no more RefCell!)
// ============================================================================

/// Outlet component that renders the matched child route at this nesting depth.
///
/// # How it works (NEW architecture)
///
/// 1. On navigation, `GlobalRouter` resolves a `MatchStack` — an ordered list
///    of matched routes from root to leaf.
/// 2. `RouterView` resets the depth counter to 0 and renders `match_stack[0]`.
/// 3. Each `RouterOutlet` claims the next depth and renders `match_stack[depth]`.
///
/// This is O(1) per outlet instead of the previous O(n) tree search.
///
/// # Example
///
/// ```ignore
/// // In parent route builder:
/// Route::new("/dashboard", |window, cx, params| {
///     div()
///         .child("Dashboard Header")
///         .child(render_router_outlet(window, cx, None)) // renders child at depth+1
/// })
/// ```
pub struct RouterOutlet {
    /// Optional outlet name (for named outlets like "sidebar")
    name: Option<String>,

    // ── REMOVED: No more RefCell<OutletState> ──
    // The outlet state (which route to render, params, etc.) is now
    // stored in the MatchStack inside GlobalRouter, not in the outlet.
}

impl RouterOutlet {
    pub fn new() -> Self {
        Self { name: None }
    }

    pub fn default() -> Self {
        Self::new()
    }

    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
        }
    }
}

impl Render for RouterOutlet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let router = cx.try_global::<GlobalRouter>();

        let Some(router) = router else {
            trace_log!("RouterOutlet: No global router found");
            return div().into_any_element();
        };

        let current_path = router.current_path().to_string();
        let stack = router.match_stack();

        // ── Named outlet: special resolution ──
        if let Some(ref name) = self.name {
            let depth = current_outlet_depth();

            let resolved = resolve_named_outlet(stack, depth, name, &current_path);

            return if let Some((route, params)) = resolved {
                route
                    .build(window, cx.deref_mut(), &params)
                    .unwrap_or_else(|| div().into_any_element())
            } else {
                trace_log!("Named outlet '{}': no matching route", name);
                div().into_any_element()
            };
        }

        // ── Default outlet: claim next depth from match stack ──
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

        // Save current depth so nested outlets inside this builder
        // will start from depth+1 (claim_outlet_depth returns depth+1)
        let saved_depth = current_outlet_depth();
        set_outlet_depth(depth);

        let element = entry
            .route
            .build(window, cx.deref_mut(), &entry.params)
            .unwrap_or_else(|| {
                div()
                    .child(format!(
                        "Route '{}' has no builder",
                        entry.route.config.path
                    ))
                    .into_any_element()
            });

        // Restore depth for sibling outlets
        set_outlet_depth(saved_depth);

        element
    }
}

// ============================================================================
// render_router_outlet — Functional API (SIMPLIFIED)
// ============================================================================

/// Render the child route at the next nesting depth.
///
/// This is the functional equivalent of `RouterOutlet`. Use it inside
/// route builders to render child content.
///
/// # Arguments
///
/// - `name`: `None` for default outlet, `Some("sidebar")` for named outlet
///
/// # Example
///
/// ```ignore
/// Route::new("/dashboard", |window, cx, params| {
///     div()
///         .child("Header")
///         .child(render_router_outlet(window, cx, None))           // main content
///         .child(render_router_outlet(window, cx, Some("sidebar"))) // sidebar
/// })
/// ```
pub fn render_router_outlet(
    window: &mut Window,
    cx: &mut App,
    name: Option<&str>,
) -> AnyElement {
    let router = cx.try_global::<GlobalRouter>();

    let Some(router) = router else {
        return div().into_any_element();
    };

    let current_path = router.current_path().to_string();
    let stack = router.match_stack();

    // ── Named outlet ──
    if let Some(name) = name {
        let depth = current_outlet_depth();

        if let Some((route, params)) = resolve_named_outlet(stack, depth, name, &current_path) {
            return route
                .build(window, cx, &params)
                .unwrap_or_else(|| div().into_any_element());
        }

        trace_log!("render_router_outlet: named outlet '{}' not found", name);
        return div().into_any_element();
    }

    // ── Default outlet ──
    let depth = claim_outlet_depth();

    let Some(entry) = stack.at_depth(depth) else {
        trace_log!(
            "render_router_outlet: no entry at depth {} (stack len={})",
            depth,
            stack.len()
        );
        return div().into_any_element();
    };

    // Set depth so nested outlets in the builder get depth+1
    let saved = current_outlet_depth();
    set_outlet_depth(depth);

    let element = entry
        .route
        .build(window, cx, &entry.params)
        .unwrap_or_else(|| div().into_any_element());

    set_outlet_depth(saved);
    element
}

// ============================================================================
// RouterView — top-level route renderer (SIMPLIFIED)
// ============================================================================

/// Renders the top-level matched route (depth 0 of the match stack).
///
/// Use this at the root of your app. It resets the outlet depth counter
/// and renders the first route in the match stack.
///
/// # Example
///
/// ```ignore
/// fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
///     div().child(router_view(window, cx))
/// }
/// ```
pub fn router_view(window: &mut Window, cx: &mut App) -> AnyElement {
    // Reset depth counter — this is the root of the render tree
    reset_outlet_depth();

    let router = cx.try_global::<GlobalRouter>();

    let Some(router) = router else {
        return div().child("No router configured").into_any_element();
    };

    let stack = router.match_stack();

    let Some(root_entry) = stack.root() else {
        let current_path = router.current_path();
        return default_not_found_page(current_path).into_any_element();
    };

    debug_log!(
        "router_view: rendering root route '{}', stack depth={}",
        root_entry.route.config.path,
        stack.len()
    );

    // Depth is 0 — nested outlets will claim 1, 2, 3...
    set_outlet_depth(0);

    root_entry
        .route
        .build(window, cx, &root_entry.params)
        .unwrap_or_else(|| {
            div()
                .child("Root route has no builder")
                .into_any_element()
        })
}

// ============================================================================
// DELETED: find_parent_route_for_path (was 140 lines)
// DELETED: find_parent_route_internal
// DELETED: OutletState, PreviousRoute
// DELETED: All RefCell-based state management in outlet
//
// These are all replaced by MatchStack + depth tracking.
// ============================================================================

// ============================================================================
// RouterLink (unchanged)
// ============================================================================

use crate::Navigator;

pub struct RouterLink {
    path: SharedString,
    active_class: Option<Box<dyn Fn(Div) -> Div>>,
    children: Vec<AnyElement>,
}

impl RouterLink {
    pub fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            active_class: None,
            children: Vec::new(),
        }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    pub fn active_class(mut self, style: impl Fn(Div) -> Div + 'static) -> Self {
        self.active_class = Some(Box::new(style));
        self
    }

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
// Default Pages (unchanged, abbreviated for clarity)
// ============================================================================

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
