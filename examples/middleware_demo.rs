//! Middleware Demo
//!
//! Demonstrates route middleware with logging, timing, and analytics.
//! A live log panel shows middleware execution order (onion model).

#![allow(clippy::needless_pass_by_ref_mut)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Entity, FontWeight, Global,
    MouseButton, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use gpui_navigator::{
    init_router, middleware_fn, NavigationRequest, Navigator, Route, RouteMiddleware, RouterOutlet,
    Transition,
};

// ============================================================================
// Shared Middleware State
// ============================================================================

#[derive(Clone)]
struct MiddlewareLog {
    entries: Arc<Mutex<Vec<LogEntry>>>,
    page_views: Arc<Mutex<HashMap<String, usize>>>,
}

impl Global for MiddlewareLog {}

struct LogEntry {
    index: usize,
    phase: String,
    middleware: String,
    path: String,
}

impl MiddlewareLog {
    fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            page_views: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn add(&self, phase: &str, middleware: &str, path: &str) {
        let mut entries = self.entries.lock().unwrap();
        let index = entries.len() + 1;
        entries.push(LogEntry {
            index,
            phase: phase.to_string(),
            middleware: middleware.to_string(),
            path: path.to_string(),
        });
        // Keep last 30 entries
        let len = entries.len();
        if len > 30 {
            entries.drain(0..len - 30);
        }
    }

    fn increment_views(&self, path: &str) {
        let mut views = self.page_views.lock().unwrap();
        *views.entry(path.to_string()).or_insert(0) += 1;
    }

    fn snapshot_entries(&self) -> Vec<(usize, String, String, String)> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e.index,
                    e.phase.clone(),
                    e.middleware.clone(),
                    e.path.clone(),
                )
            })
            .collect()
    }

    fn snapshot_views(&self) -> Vec<(String, usize)> {
        let mut views: Vec<_> = self
            .page_views
            .lock()
            .unwrap()
            .clone()
            .into_iter()
            .collect();
        views.sort_by(|a, b| a.0.cmp(&b.0));
        views
    }
}

// ============================================================================
// Middleware Implementations
// ============================================================================

/// Struct-based logging middleware with high priority (runs first in before, last in after).
struct LoggingMiddleware {
    log: MiddlewareLog,
}

impl RouteMiddleware for LoggingMiddleware {
    fn before_navigation(&self, _cx: &App, request: &NavigationRequest) {
        self.log.add("BEFORE", "Logging", &request.to);
    }

    fn after_navigation(&self, _cx: &App, request: &NavigationRequest) {
        self.log.add("AFTER", "Logging", &request.to);
    }

    fn name(&self) -> &'static str {
        "LoggingMiddleware"
    }

    fn priority(&self) -> i32 {
        100
    }
}

/// Struct-based timing middleware with medium priority.
struct TimingMiddleware {
    log: MiddlewareLog,
    start: Arc<Mutex<Option<Instant>>>,
}

impl TimingMiddleware {
    fn new(log: MiddlewareLog) -> Self {
        Self {
            log,
            start: Arc::new(Mutex::new(None)),
        }
    }
}

impl RouteMiddleware for TimingMiddleware {
    fn before_navigation(&self, _cx: &App, request: &NavigationRequest) {
        *self.start.lock().unwrap() = Some(Instant::now());
        self.log.add("BEFORE", "Timing (start)", &request.to);
    }

    fn after_navigation(&self, _cx: &App, request: &NavigationRequest) {
        let elapsed = self
            .start
            .lock()
            .unwrap()
            .map(|s| s.elapsed().as_micros())
            .unwrap_or(0);
        self.log
            .add("AFTER", &format!("Timing ({elapsed}us)"), &request.to);
    }

    fn name(&self) -> &'static str {
        "TimingMiddleware"
    }

    fn priority(&self) -> i32 {
        50
    }
}

// ============================================================================
// Main
// ============================================================================

/// Create an analytics middleware instance from a log.
fn make_analytics(log: &MiddlewareLog) -> impl RouteMiddleware {
    let before_log = log.clone();
    let after_log = log.clone();
    middleware_fn(
        move |_cx: &App, request: &NavigationRequest| {
            before_log.add("BEFORE", "Analytics", &request.to);
        },
        move |_cx: &App, request: &NavigationRequest| {
            after_log.increment_views(&request.to);
            after_log.add("AFTER", "Analytics (counted)", &request.to);
        },
    )
}

fn setup_routes(log: &MiddlewareLog, cx: &mut App) {
    let logging1 = LoggingMiddleware { log: log.clone() };
    let logging2 = LoggingMiddleware { log: log.clone() };
    let logging3 = LoggingMiddleware { log: log.clone() };
    let logging4 = LoggingMiddleware { log: log.clone() };
    let timing = TimingMiddleware::new(log.clone());

    init_router(cx, |router| {
        router.add_route(
            Route::new("/", |_, _, _| {
                content_page(
                    "Home",
                    "Middleware: Logging (100) + Analytics (0)",
                    rgb(0x21_96_f3),
                )
                .into_any_element()
            })
            .middleware(logging1)
            .middleware(make_analytics(log))
            .transition(Transition::fade(200)),
        );

        router.add_route(
            Route::new("/products", |_, _, _| {
                content_page(
                    "Products",
                    "Middleware: Logging (100) + Timing (50) + Analytics (0)",
                    rgb(0x4c_af_50),
                )
                .into_any_element()
            })
            .middleware(logging2)
            .middleware(timing)
            .middleware(make_analytics(log))
            .transition(Transition::slide_left(300)),
        );

        router.add_route(
            Route::new("/about", |_, _, _| {
                content_page(
                    "About",
                    "Middleware: Logging (100) + Analytics (0)",
                    rgb(0x9c_27_b0),
                )
                .into_any_element()
            })
            .middleware(logging3)
            .middleware(make_analytics(log))
            .transition(Transition::slide_right(300)),
        );

        router.add_route(
            Route::new("/settings", |_, _, _| {
                content_page(
                    "Settings",
                    "Middleware: Logging only (no analytics tracking here)",
                    rgb(0xff_98_00),
                )
                .into_any_element()
            })
            .middleware(logging4)
            .transition(Transition::fade(200)),
        );
    });
}

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        let log = MiddlewareLog::new();
        cx.set_global(log.clone());
        setup_routes(&log, cx);

        let bounds = Bounds::centered(None, size(px(1000.), px(750.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Middleware Demo".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(MiddlewareDemoApp::new),
        )
        .unwrap();

        cx.activate(true);
    });
}

// ============================================================================
// Root App Component
// ============================================================================

struct MiddlewareDemoApp {
    outlet: Entity<RouterOutlet>,
}

impl MiddlewareDemoApp {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            outlet: cx.new(|_| RouterOutlet::new()),
        }
    }
}

impl Render for MiddlewareDemoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let current_path = Navigator::current_path(cx);
        let log = cx.global::<MiddlewareLog>();
        let entries = log.snapshot_entries();
        let views = log.snapshot_views();

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
                            .child("Middleware Demo"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x88_88_88))
                            .child(format!("Path: {current_path}")),
                    ),
            )
            // Body: sidebar + content + log panel
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Sidebar
                    .child(
                        div()
                            .w(px(200.))
                            .bg(rgb(0x25_25_26))
                            .border_r_1()
                            .border_color(rgb(0x3e_3e_3e))
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xcc_cc_cc))
                                    .mb_1()
                                    .child("Navigation"),
                            )
                            .child(self.nav_button(cx, "/", "Home", &current_path))
                            .child(self.nav_button(cx, "/products", "Products", &current_path))
                            .child(self.nav_button(cx, "/about", "About", &current_path))
                            .child(self.nav_button(cx, "/settings", "Settings", &current_path))
                            .child(div().h_px().bg(rgb(0x3e_3e_3e)).my_2())
                            // Page views
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xcc_cc_cc))
                                    .mb_1()
                                    .child("Page Views"),
                            )
                            .children(views.iter().map(|(path, count)| {
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_xs()
                                    .text_color(rgb(0x88_88_88))
                                    .child(path.clone())
                                    .child(
                                        div().text_color(rgb(0x4e_c9_b0)).child(count.to_string()),
                                    )
                            })),
                    )
                    // Content
                    .child(div().flex_1().child(self.outlet.clone())),
            )
            // Log panel at bottom
            .child(log_panel(&entries))
    }
}

impl MiddlewareDemoApp {
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

// ============================================================================
// Log Panel
// ============================================================================

fn log_panel(entries: &[(usize, String, String, String)]) -> impl IntoElement {
    let mut panel = div()
        .h(px(200.))
        .bg(rgb(0x1a_1a_2e))
        .border_t_1()
        .border_color(rgb(0x3e_3e_3e))
        .p_3()
        .flex()
        .flex_col()
        .gap_1()
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .mb_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xcc_cc_cc))
                        .child("Middleware Execution Log"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x66_66_66))
                        .child("(BEFORE: high priority first, AFTER: low priority first)"),
                ),
        );

    // Show last ~8 entries
    let display_entries: Vec<_> = entries.iter().rev().take(8).collect();
    for (idx, phase, middleware, path) in display_entries.into_iter().rev() {
        let phase_color = if phase == "BEFORE" {
            rgb(0x4e_c9_b0)
        } else {
            rgb(0xdc_dc_aa)
        };

        panel = panel.child(
            div()
                .flex()
                .gap_2()
                .text_xs()
                .child(
                    div()
                        .text_color(rgb(0x55_55_55))
                        .w(px(24.))
                        .child(format!("#{idx}")),
                )
                .child(
                    div()
                        .text_color(phase_color)
                        .w(px(55.))
                        .child(phase.clone()),
                )
                .child(
                    div()
                        .text_color(rgb(0xce_91_78))
                        .w(px(160.))
                        .child(middleware.clone()),
                )
                .child(div().text_color(rgb(0x88_88_88)).child(path.clone())),
        );
    }

    if entries.is_empty() {
        panel = panel.child(
            div()
                .text_xs()
                .text_color(rgb(0x55_55_55))
                .child("Navigate between pages to see middleware execution..."),
        );
    }

    panel
}

// ============================================================================
// Pages
// ============================================================================

fn content_page(title: &str, description: &str, accent: gpui::Rgba) -> impl IntoElement {
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
                .child(div().w_4().h(px(40.)).rounded_md().bg(accent))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .child(title.to_string()),
                ),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xaa_aa_aa))
                .max_w(px(600.))
                .child(description.to_string()),
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
                        .child("Navigate to other pages and watch the log panel below."),
                ),
        )
}
