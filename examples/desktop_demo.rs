//! Desktop navigation demo
//!
//! Shows the pieces a desktop app usually wants on top of plain routing:
//!
//! - typed routes (an enum instead of path strings)
//! - back/forward with `alt-left` / `alt-right` (`cmd-[` / `cmd-]` on macOS)
//!   and the mouse's side buttons
//! - the window title following the current route (`on_route_change`)
//! - an async guard that "checks the session" before opening the account page
//! - a page that refuses to be left while it has unsaved changes (`can_leave`)
//! - query strings, a `*` catch-all 404 page, sidebar links active for a
//!   whole section (`ActiveMatch::Prefix`) and the router debug panel
//!
//! Run with `cargo run --example desktop_demo`.

#![allow(clippy::needless_pass_by_ref_mut)]

use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, Application, Bounds, Context, SharedString, Window, WindowBounds,
    WindowOptions,
};
use gpui_navigator::{
    async_guard_fn, bind_navigation_keys, init_router, on_route_change, router_debug_panel,
    router_view, with_mouse_navigation, ActiveMatch, IntoRoute, NavigationAction, Navigator, Route,
    RouteDescriptor, RouterLink, Transition,
};
use std::time::Duration;

// ============================================================================
// Typed routes
// ============================================================================

/// Every screen of the app. Navigating with this enum instead of strings
/// turns a typo in a path into a compile error.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AppRoute {
    Home,
    Projects { page: u32 },
    Project { id: u32 },
    Notes,
    Account,
}

impl AppRoute {
    fn path(&self) -> String {
        match self {
            Self::Home => "/".into(),
            Self::Projects { page } => format!("/projects?page={page}"),
            Self::Project { id } => format!("/projects/{id}"),
            Self::Notes => "/notes".into(),
            Self::Account => "/account".into(),
        }
    }
}

impl IntoRoute for AppRoute {
    fn into_route(self) -> RouteDescriptor {
        self.path().into_route()
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        init_router(cx, |router| {
            router.add_route(Route::view("/", home_page).transition(Transition::fade(150)));
            router.add_route(Route::new("/projects", |_, cx, _| projects_page(cx)));
            router.add_route(
                Route::new("/projects/:id<u32>", |_, _, params| {
                    let id: u32 = params.get_as("id").unwrap_or_default();
                    project_page(id)
                })
                .transition(
                    Transition::slide_left(220).with_easing(gpui_navigator::ease_out_cubic),
                ),
            );
            router.add_route(
                Route::component_with_context("/notes", NotesPage::new).can_leave(
                    |notes: &NotesPage, _| {
                        if notes.dirty {
                            NavigationAction::deny("Notes have unsaved changes")
                        } else {
                            NavigationAction::Continue
                        }
                    },
                ),
            );
            router.add_route(Route::view("/account", account_page).guard(async_guard_fn(
                |cx, _request| {
                    // Pretend to ask a server whether the session is still valid.
                    let check = cx.background_executor().timer(Duration::from_millis(600));
                    async move {
                        check.await;
                        NavigationAction::Continue
                    }
                },
            )));
            router.add_route(Route::new("*", |_, cx, _| not_found_page(cx)));
        });

        bind_navigation_keys(cx);

        let bounds = Bounds::centered(None, size(px(1000.), px(680.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| AppShell),
            )
            .unwrap();

        // Keep the window title in sync with the route.
        on_route_change(cx, move |event, cx| {
            let title = format!("gpui-navigator · {}", event.to);
            let _ = window.update(cx, |_, window, _| window.set_window_title(&title));
        })
        .detach();
    });
}

// ============================================================================
// Shell
// ============================================================================

struct AppShell;

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let pending = Navigator::is_pending(cx);
        with_mouse_navigation(div().size_full().flex().bg(rgb(0x00f5_f5f5)))
            .child(sidebar(cx, pending))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(div().flex_1().p_6().child(router_view(window, cx)))
                    .child(router_debug_panel(cx)),
            )
    }
}

fn sidebar(cx: &mut Context<'_, AppShell>, pending: bool) -> impl IntoElement {
    let link = |cx: &mut Context<'_, AppShell>, route: AppRoute, label: &str, mode| {
        RouterLink::new(SharedString::from(route.path()))
            .active_match(mode)
            .active_class(|d| d.bg(rgb(0x00dd_e7ff)).text_color(rgb(0x001d_4ed8)))
            .child(label.to_string())
            .build(cx)
            .px_3()
            .py_2()
            .rounded_md()
    };

    div()
        .w(px(200.))
        .flex()
        .flex_col()
        .gap_1()
        .p_3()
        .bg(rgb(0x00ff_ffff))
        .child(link(cx, AppRoute::Home, "Home", ActiveMatch::Exact))
        .child(link(
            cx,
            AppRoute::Projects { page: 1 },
            "Projects",
            ActiveMatch::Prefix,
        ))
        .child(link(cx, AppRoute::Notes, "Notes", ActiveMatch::Exact))
        .child(link(cx, AppRoute::Account, "Account", ActiveMatch::Exact))
        .child(
            div()
                .mt_4()
                .text_xs()
                .text_color(rgb(0x0066_6666))
                .child(if pending {
                    "Checking session…"
                } else {
                    "alt-left / alt-right: back and forward"
                }),
        )
}

// ============================================================================
// Pages
// ============================================================================

fn home_page() -> gpui::AnyElement {
    div()
        .child("Home")
        .child("Use the sidebar, the keyboard or the mouse side buttons.")
        .into_any_element()
}

fn projects_page(cx: &App) -> gpui::AnyElement {
    let page = Navigator::query(cx).get_as::<u32>("page").unwrap_or(1);
    let mut list = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(format!("Projects, page {page}"));
    for id in (page - 1) * 5 + 1..=page * 5 {
        list = list.child(
            div()
                .id(SharedString::from(format!("project-{id}")))
                .cursor_pointer()
                .child(format!("Project #{id}"))
                .on_click(move |_, _, cx| Navigator::push(cx, AppRoute::Project { id })),
        );
    }
    list.child(
        div()
            .id("next-page")
            .cursor_pointer()
            .mt_2()
            .child("Next page →")
            .on_click(move |_, _, cx| {
                Navigator::replace(cx, AppRoute::Projects { page: page + 1 });
            }),
    )
    .into_any_element()
}

fn project_page(id: u32) -> gpui::AnyElement {
    div()
        .child(format!("Project #{id}"))
        .child("alt-left goes back to the list.")
        .into_any_element()
}

fn account_page() -> gpui::AnyElement {
    div()
        .child("Account")
        .child("Reached after the async session check.")
        .into_any_element()
}

fn not_found_page(cx: &App) -> gpui::AnyElement {
    div()
        .child(format!("Nothing at {}", Navigator::current_path(cx)))
        .into_any_element()
}

/// A page with state the router asks about before leaving.
struct NotesPage {
    dirty: bool,
}

impl NotesPage {
    const fn new(_window: &mut Window, _cx: &mut Context<'_, Self>) -> Self {
        Self { dirty: false }
    }
}

impl Render for NotesPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let label = if self.dirty {
            "Unsaved changes: leaving is blocked. Click to save."
        } else {
            "Saved. Click to make a change."
        };
        div().child("Notes").child(
            div()
                .id("toggle-dirty")
                .cursor_pointer()
                .child(label)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.dirty = !this.dirty;
                    cx.notify();
                })),
        )
    }
}
