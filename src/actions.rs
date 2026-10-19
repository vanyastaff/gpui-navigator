//! Desktop navigation input: back/forward actions, key bindings, mouse
//! side buttons, and route change notifications.
//!
//! ```no_run
//! use gpui::*;
//! use gpui_navigator::{bind_navigation_keys, init_router, on_route_change, Route};
//!
//! Application::new().run(|cx: &mut App| {
//!     init_router(cx, |router| {
//!         router.add_route(Route::view("/", || div().into_any_element()));
//!     });
//!
//!     // alt-left / alt-right (and cmd-[ / cmd-] on macOS) go back and forward
//!     bind_navigation_keys(cx);
//!
//!     // Keep the window title, analytics, breadcrumbs... in sync
//!     on_route_change(cx, |event, _cx| println!("now at {}", event.to)).detach();
//! });
//! ```

// The derives come from GPUI's `actions!` macro.
#![allow(clippy::derive_partial_eq_without_eq)]

use crate::context::GlobalRouter;
use crate::{Navigator, RouteChangeEvent};
use gpui::{
    actions, App, InteractiveElement, KeyBinding, MouseButton, NavigationDirection, Subscription,
};
use std::cell::Cell;
use std::rc::Rc;

actions!(
    navigator,
    [
        /// Go back in the router history.
        GoBack,
        /// Go forward in the router history.
        GoForward
    ]
);

/// Register handlers for [`GoBack`] / [`GoForward`] and bind the platform's
/// usual back/forward shortcuts to them.
///
/// Bindings: `alt-left` / `alt-right` everywhere, plus `cmd-[` / `cmd-]` on
/// macOS. The actions are handled globally, so they work in every window
/// unless a focused element handles them first. To pick other keys, call
/// [`register_navigation_actions`] and bind [`GoBack`] / [`GoForward`]
/// yourself.
pub fn bind_navigation_keys(cx: &mut App) {
    register_navigation_actions(cx);
    let mut bindings = vec![
        KeyBinding::new("alt-left", GoBack, None),
        KeyBinding::new("alt-right", GoForward, None),
    ];
    if cfg!(target_os = "macos") {
        bindings.push(KeyBinding::new("cmd-[", GoBack, None));
        bindings.push(KeyBinding::new("cmd-]", GoForward, None));
    }
    cx.bind_keys(bindings);
}

/// Register global handlers for [`GoBack`] and [`GoForward`] without binding keys.
pub fn register_navigation_actions(cx: &mut App) {
    cx.on_action(|_: &GoBack, cx: &mut App| Navigator::back(cx));
    cx.on_action(|_: &GoForward, cx: &mut App| Navigator::forward(cx));
}

/// Make the mouse's back/forward side buttons navigate while the pointer is
/// over `element` (usually your window's root view).
///
/// ```no_run
/// use gpui::*;
/// use gpui_navigator::{router_view, with_mouse_navigation};
///
/// struct AppShell;
///
/// impl Render for AppShell {
///     fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
///         with_mouse_navigation(div().size_full()).child(router_view(window, cx))
///     }
/// }
/// ```
pub fn with_mouse_navigation<E: InteractiveElement>(element: E) -> E {
    element
        .on_mouse_down(
            MouseButton::Navigate(NavigationDirection::Back),
            |_, _, cx| Navigator::back(cx),
        )
        .on_mouse_down(
            MouseButton::Navigate(NavigationDirection::Forward),
            |_, _, cx| Navigator::forward(cx),
        )
}

/// Call `callback` after every completed navigation.
///
/// Fires once per navigation (not on re-renders), after guards, hooks and
/// middleware ran. Keep the returned [`Subscription`] alive, or call
/// `.detach()` to keep the callback for the lifetime of the app.
///
/// Requires [`init_router`](crate::init_router) to have been called first.
pub fn on_route_change(
    cx: &mut App,
    mut callback: impl FnMut(&RouteChangeEvent, &mut App) + 'static,
) -> Subscription {
    let seen = Rc::new(Cell::new(
        cx.try_global::<GlobalRouter>()
            .map_or(0, GlobalRouter::change_count),
    ));
    cx.observe_global::<GlobalRouter>(move |cx| {
        let router = cx.global::<GlobalRouter>();
        let count = router.change_count();
        if count == seen.get() {
            return;
        }
        seen.set(count);
        if let Some(event) = router.last_change().cloned() {
            callback(&event, cx);
        }
    })
}
