//! gpui-navigator + gpui-component
//!
//! A typical app layout built from gpui-component widgets:
//!
//! - a sidebar of `Button`s that highlight the active section
//! - a settings section with nested routes rendered through an outlet
//! - a form page whose `Input` keeps its text across navigation
//!   (`Route::component_with_context`) and refuses to be left with unsaved
//!   edits (`Route::can_leave`)
//!
//! Run from this directory with `cargo run`.

use gpui::prelude::*;
use gpui::{
    div, px, size, App, Application, Bounds, Context, Entity, SharedString, Window, WindowBounds,
    WindowOptions,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, Root};
use gpui_navigator::{
    bind_navigation_keys, init_router, is_link_active, render_router_outlet, router_view,
    with_mouse_navigation, ActiveMatch, NavigationAction, Navigator, Route,
};

fn main() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);

        init_router(cx, |router| {
            router.add_route(Route::view("/", || {
                div()
                    .child("Welcome! Pick a section on the left.")
                    .into_any_element()
            }));
            router.add_route(
                Route::component_with_context("/profile", ProfileForm::new).can_leave(
                    |form: &ProfileForm, _| {
                        if form.dirty {
                            NavigationAction::deny("Save or discard your changes first")
                        } else {
                            NavigationAction::Continue
                        }
                    },
                ),
            );
            router.add_route(
                Route::new("/settings", |window, cx, _| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(nav_button(
                                    "settings-general",
                                    "General",
                                    "/settings",
                                    ActiveMatch::Exact,
                                    cx,
                                ))
                                .child(nav_button(
                                    "settings-keys",
                                    "Shortcuts",
                                    "/settings/shortcuts",
                                    ActiveMatch::Exact,
                                    cx,
                                )),
                        )
                        .child(render_router_outlet(window, cx, None))
                        .into_any_element()
                })
                .children(vec![
                    Route::view("", || div().child("General settings").into_any_element()).into(),
                    Route::view("shortcuts", || {
                        div()
                            .child("alt-left / alt-right go back and forward")
                            .into_any_element()
                    })
                    .into(),
                ]),
            );
        });
        bind_navigation_keys(cx);

        let bounds = Bounds::centered(None, size(px(900.), px(600.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let shell = cx.new(|_| AppShell);
                // gpui-component widgets expect `Root` as the window's root view.
                cx.new(|cx| Root::new(shell, window, cx))
            },
        )
        .unwrap();
    });
}

/// A sidebar/tab button that navigates and highlights when its route is active.
fn nav_button(
    id: &'static str,
    label: &'static str,
    path: &'static str,
    mode: ActiveMatch,
    cx: &App,
) -> Button {
    let active = is_link_active(&Navigator::current_path(cx), path, mode);
    let button = Button::new(id)
        .label(label)
        .on_click(move |_, _, cx| Navigator::push(cx, path));
    if active {
        button.primary()
    } else {
        button.ghost()
    }
}

struct AppShell;

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let sidebar = div()
            .w(px(180.))
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(nav_button("nav-home", "Home", "/", ActiveMatch::Exact, cx))
            .child(nav_button(
                "nav-profile",
                "Profile",
                "/profile",
                ActiveMatch::Exact,
                cx,
            ))
            .child(nav_button(
                "nav-settings",
                "Settings",
                "/settings",
                ActiveMatch::Prefix,
                cx,
            ));

        with_mouse_navigation(
            div()
                .size_full()
                .flex()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground),
        )
        .child(sidebar)
        .child(div().flex_1().p_6().child(router_view(window, cx)))
    }
}

/// A form page: its `InputState` is created once and cached by the router.
struct ProfileForm {
    name: Entity<InputState>,
    saved: SharedString,
    dirty: bool,
}

impl ProfileForm {
    fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Your name"));
        cx.subscribe(&name, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.dirty = true;
                cx.notify();
            }
        })
        .detach();
        Self {
            name,
            saved: SharedString::default(),
            dirty: false,
        }
    }
}

impl Render for ProfileForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .w(px(360.))
            .child(Input::new(&self.name))
            .child(
                Button::new("save")
                    .primary()
                    .label("Save")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.saved = this.name.read(cx).value();
                        this.dirty = false;
                        cx.notify();
                    })),
            )
            .child(if self.dirty {
                "Unsaved changes: navigation away is blocked".to_string()
            } else if self.saved.is_empty() {
                "Nothing saved yet".to_string()
            } else {
                format!("Saved: {}", self.saved)
            })
    }
}
