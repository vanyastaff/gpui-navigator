//! Lifecycle Hooks Demo
//!
//! Demonstrates `RouteLifecycle` hooks: `can_deactivate`, `on_enter`, `on_exit`.
//! A form editor blocks navigation when it has unsaved changes.

#![allow(clippy::needless_pass_by_ref_mut)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Entity, FontWeight, Global,
    MouseButton, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use gpui_navigator::{
    init_router, NavigationAction, NavigationRequest, Navigator, Route, RouteLifecycle,
    RouterOutlet, Transition,
};

// ============================================================================
// Lifecycle State — shared via Arc for thread safety
// ============================================================================

struct LifecycleState {
    has_unsaved_changes: AtomicBool,
    enter_count: AtomicUsize,
    exit_count: AtomicUsize,
    last_event: std::sync::Mutex<String>,
    blocked_count: AtomicUsize,
}

impl Global for LifecycleState {}

impl LifecycleState {
    fn new() -> Self {
        Self {
            has_unsaved_changes: AtomicBool::new(false),
            enter_count: AtomicUsize::new(0),
            exit_count: AtomicUsize::new(0),
            last_event: std::sync::Mutex::new("(none)".to_string()),
            blocked_count: AtomicUsize::new(0),
        }
    }

    fn is_dirty(&self) -> bool {
        self.has_unsaved_changes.load(Ordering::Relaxed)
    }

    fn log_event(&self, event: &str) {
        *self.last_event.lock().unwrap() = event.to_string();
    }

    fn last_event(&self) -> String {
        self.last_event.lock().unwrap().clone()
    }
}

// ============================================================================
// Lifecycle Implementations
// ============================================================================

/// Form lifecycle: blocks navigation when unsaved changes exist.
struct FormLifecycle {
    state: Arc<LifecycleState>,
}

impl RouteLifecycle for FormLifecycle {
    fn on_enter(&self, _cx: &App, _request: &NavigationRequest) -> NavigationAction {
        let count = self.state.enter_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.state
            .log_event(&format!("on_enter: Editor opened (#{count})"));
        NavigationAction::Continue
    }

    fn on_exit(&self, _cx: &App) -> NavigationAction {
        let count = self.state.exit_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.state
            .log_event(&format!("on_exit: Editor cleanup (#{count})"));
        NavigationAction::Continue
    }

    fn can_deactivate(&self, _cx: &App) -> NavigationAction {
        if self.state.is_dirty() {
            self.state.blocked_count.fetch_add(1, Ordering::Relaxed);
            self.state
                .log_event("can_deactivate: BLOCKED (unsaved changes)");
            NavigationAction::deny("You have unsaved changes! Save or discard before leaving.")
        } else {
            self.state.log_event("can_deactivate: allowed");
            NavigationAction::Continue
        }
    }
}

/// Preview lifecycle: just logs `on_enter`.
struct PreviewLifecycle {
    state: Arc<LifecycleState>,
}

impl RouteLifecycle for PreviewLifecycle {
    fn on_enter(&self, _cx: &App, _request: &NavigationRequest) -> NavigationAction {
        self.state.log_event("on_enter: Preview loaded");
        NavigationAction::Continue
    }

    fn on_exit(&self, _cx: &App) -> NavigationAction {
        self.state.log_event("on_exit: Preview unloaded");
        NavigationAction::Continue
    }

    fn can_deactivate(&self, _cx: &App) -> NavigationAction {
        NavigationAction::Continue
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        let state = Arc::new(LifecycleState::new());
        cx.set_global(LifecycleState::new());

        let form_lifecycle = FormLifecycle {
            state: state.clone(),
        };
        let preview_lifecycle = PreviewLifecycle {
            state: state.clone(),
        };

        // Store the Arc in a global so pages can read it
        cx.set_global(SharedLifecycleState(state));

        init_router(cx, |router| {
            router.add_route(
                Route::new("/", |_, _, _| home_page().into_any_element())
                    .transition(Transition::fade(200)),
            );

            router.add_route(
                Route::new("/editor", |_, cx, _| editor_page(cx).into_any_element())
                    .lifecycle(form_lifecycle)
                    .transition(Transition::slide_left(300)),
            );

            router.add_route(
                Route::new("/preview", |_, _, _| preview_page().into_any_element())
                    .lifecycle(preview_lifecycle)
                    .transition(Transition::slide_right(300)),
            );

            router.add_route(
                Route::new("/saved", |_, _, _| saved_page().into_any_element())
                    .transition(Transition::fade(200)),
            );
        });

        let bounds = Bounds::centered(None, size(px(900.), px(650.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Lifecycle Hooks Demo".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(LifecycleDemoApp::new),
        )
        .unwrap();

        cx.activate(true);
    });
}

// ============================================================================
// Shared state wrapper for GPUI Global
// ============================================================================

struct SharedLifecycleState(Arc<LifecycleState>);
impl Global for SharedLifecycleState {}

// ============================================================================
// Root App Component
// ============================================================================

struct LifecycleDemoApp {
    outlet: Entity<RouterOutlet>,
}

impl LifecycleDemoApp {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            outlet: cx.new(|_| RouterOutlet::new()),
        }
    }
}

impl Render for LifecycleDemoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let current_path = Navigator::current_path(cx);
        let state = &cx.global::<SharedLifecycleState>().0;
        let is_dirty = state.is_dirty();
        let enter_count = state.enter_count.load(Ordering::Relaxed);
        let exit_count = state.exit_count.load(Ordering::Relaxed);
        let blocked = state.blocked_count.load(Ordering::Relaxed);
        let last_event = state.last_event();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e_1e_1e))
            .text_color(rgb(0xff_ff_ff))
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_4()
                    .bg(rgb(0x2d_2d_2d))
                    .border_b_1()
                    .border_color(rgb(0x3e_3e_3e))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .child("Lifecycle Hooks Demo"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x88_88_88))
                            .child(format!("Path: {current_path}")),
                    ),
            )
            // Body
            .child(
                div()
                    .flex()
                    .flex_1()
                    // Sidebar
                    .child(
                        div()
                            .w(px(240.))
                            .bg(rgb(0x25_25_26))
                            .border_r_1()
                            .border_color(rgb(0x3e_3e_3e))
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_2()
                            // Navigation
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xcc_cc_cc))
                                    .mb_1()
                                    .child("Navigation"),
                            )
                            .child(self.nav_button(cx, "/", "Home", &current_path))
                            .child(self.nav_button(
                                cx,
                                "/editor",
                                "Editor (lifecycle)",
                                &current_path,
                            ))
                            .child(self.nav_button(
                                cx,
                                "/preview",
                                "Preview (lifecycle)",
                                &current_path,
                            ))
                            .child(self.nav_button(cx, "/saved", "Saved", &current_path))
                            .child(div().h_px().bg(rgb(0x3e_3e_3e)).my_2())
                            // Stats
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xcc_cc_cc))
                                    .mb_1()
                                    .child("Lifecycle Stats"),
                            )
                            .child(stat_row("on_enter calls:", &enter_count.to_string()))
                            .child(stat_row("on_exit calls:", &exit_count.to_string()))
                            .child(stat_row("Blocked nav:", &blocked.to_string()))
                            .child(stat_row("Dirty:", if is_dirty { "YES" } else { "no" }))
                            .child(div().h_px().bg(rgb(0x3e_3e_3e)).my_2())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x88_88_88))
                                    .child("Last event:"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x4e_c9_b0))
                                    .child(last_event),
                            ),
                    )
                    // Content
                    .child(div().flex_1().child(self.outlet.clone())),
            )
    }
}

impl LifecycleDemoApp {
    #[allow(clippy::unused_self)]
    fn nav_button(
        &self,
        cx: &mut Context<'_, Self>,
        path: &str,
        label: &str,
        current_path: &str,
    ) -> impl IntoElement {
        let is_active = current_path == path;
        let path = path.to_string();
        let label_str = label.to_string();

        div()
            .id(SharedString::from(format!("nav-{label_str}")))
            .px_3()
            .py_2()
            .rounded_md()
            .text_sm()
            .cursor_pointer()
            .when(is_active, |this| {
                this.bg(rgb(0x09_47_71)).text_color(rgb(0xff_ff_ff))
            })
            .when(!is_active, |this| {
                this.text_color(rgb(0xcc_cc_cc))
                    .hover(|this| this.bg(rgb(0x2a_2d_2e)))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_view, _event, _window, cx| {
                    Navigator::push(cx, path.clone());
                }),
            )
            .child(label_str)
    }
}

fn stat_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .text_xs()
        .child(div().text_color(rgb(0x88_88_88)).child(label.to_string()))
        .child(div().text_color(rgb(0x4e_c9_b0)).child(value.to_string()))
}

// ============================================================================
// Pages
// ============================================================================

fn home_page() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .p_8()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(div().w_4().h(px(40.)).rounded_md().bg(rgb(0x21_96_f3)))
                .child(div().text_2xl().font_weight(FontWeight::BOLD).child("Home")),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xaa_aa_aa))
                .max_w(px(600.))
                .child("Navigate to the Editor to see lifecycle hooks in action."),
        )
        .child(
            div()
                .mt_4()
                .p_4()
                .bg(rgb(0x2d_2d_2d))
                .rounded_md()
                .border_1()
                .border_color(rgb(0x3e_3e_3e))
                .flex()
                .flex_col()
                .gap_2()
                .child("How it works:")
                .child("  1. Navigate to Editor - on_enter fires")
                .child("  2. Toggle 'dirty' state in the editor")
                .child("  3. Try to navigate away - can_deactivate blocks you")
                .child("  4. Save or discard changes, then navigate freely"),
        )
}

fn editor_page(cx: &App) -> impl IntoElement {
    let state = &cx.global::<SharedLifecycleState>().0;
    let is_dirty = state.is_dirty();

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_8()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(div().w_4().h(px(40.)).rounded_md().bg(rgb(0x4c_af_50)))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .child("Editor"),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_xs()
                        .bg(if is_dirty {
                            rgb(0x4a_14_14)
                        } else {
                            rgb(0x1b_5e_20)
                        })
                        .child(if is_dirty { "UNSAVED" } else { "CLEAN" }),
                ),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xaa_aa_aa))
                .child(if is_dirty {
                    "This form has unsaved changes. Navigation will be blocked by can_deactivate."
                } else {
                    "Edit the form below. When dirty, you won't be able to leave without saving."
                }),
        )
        // Simulated form
        .child(
            div()
                .mt_4()
                .p_6()
                .bg(rgb(0x2d_2d_2d))
                .rounded_md()
                .border_1()
                .border_color(rgb(0x3e_3e_3e))
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xcc_cc_cc))
                        .child("Form Controls"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x88_88_88))
                        .child("(In a real app, editing fields would set dirty=true)"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x88_88_88))
                        .child("Use the buttons below to simulate form state changes:"),
                ),
        )
        // Action note
        .child(
            div()
                .mt_2()
                .p_3()
                .bg(if is_dirty {
                    rgb(0x3e_21_21)
                } else {
                    rgb(0x1a_2e_1a)
                })
                .rounded_md()
                .text_sm()
                .child(if is_dirty {
                    "Try clicking any nav link - navigation will be blocked!"
                } else {
                    "Form is clean. You can navigate freely."
                }),
        )
}

fn preview_page() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .p_8()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(div().w_4().h(px(40.)).rounded_md().bg(rgb(0x9c_27_b0)))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .child("Preview"),
                ),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xaa_aa_aa))
                .child("This page has a PreviewLifecycle that logs on_enter and on_exit."),
        )
        .child(
            div()
                .mt_4()
                .p_4()
                .bg(rgb(0x2d_2d_2d))
                .rounded_md()
                .border_1()
                .border_color(rgb(0x3e_3e_3e))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x88_88_88))
                        .child("Check the sidebar stats - on_enter incremented when you arrived."),
                ),
        )
}

fn saved_page() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .p_8()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(div().w_4().h(px(40.)).rounded_md().bg(rgb(0xff_98_00)))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .child("Saved!"),
                ),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xaa_aa_aa))
                .child("Changes saved successfully. Go back to the editor to try again."),
        )
}
