//! Error Handlers Demo - `RouterLink` Example
//!
//! Demonstrates `RouterLink` usage with valid and invalid routes.

use gpui::prelude::*;
use gpui::{
    div, px, relative, rgb, size, App, AppContext, Application, Bounds, Div, Entity, FontWeight,
    Rgba, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use gpui_navigator::{
    init_router, Navigator, Route, RouteParams, RouterLink, RouterOutlet, Transition,
};

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        // Initialize router
        init_router(cx, |router| {
            router.add_route(
                Route::new("/", |_, _, _| home_page().into_any_element())
                    .transition(Transition::fade(200)),
            );
            router.add_route(
                Route::new("/about", |_, _, _| about_page().into_any_element())
                    .transition(Transition::slide_left(300)),
            );
            router.add_route(
                Route::new("/users/:id", |_, _, params| {
                    user_page(params).into_any_element()
                })
                .transition(Transition::slide_right(300)),
            );
        });

        let bounds = Bounds::centered(None, size(px(1000.), px(700.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("RouterLink Demo - Error Handling".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(DemoApp::new),
        )
        .unwrap();

        cx.activate(true);
    });
}

struct DemoApp {
    outlet: Entity<RouterOutlet>,
}

impl DemoApp {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            outlet: cx.new(|_| RouterOutlet::new()),
        }
    }
}

impl Render for DemoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .child(header(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .child(sidebar(cx))
                    .child(div().flex_1().child(self.outlet.clone())),
            )
    }
}

fn header(cx: &mut Context<'_, DemoApp>) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .p_4()
        .bg(rgb(0x2d2d2d))
        .border_b_1()
        .border_color(rgb(0x3e3e3e))
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child("RouterLink Demo"),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(div().text_sm().text_color(rgb(0x888888)).child("Path:"))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x4ec9b0))
                        .child(Navigator::current_path(cx)),
                ),
        )
}

fn sidebar(cx: &mut Context<'_, DemoApp>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_64()
        .bg(rgb(0x252526))
        .border_r_1()
        .border_color(rgb(0x3e3e3e))
        .p_4()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xcccccc))
                .mb_2()
                .child("Valid Routes"),
        )
        .child(nav_link(cx, "/", "Home"))
        .child(nav_link(cx, "/about", "About"))
        .child(nav_link(cx, "/users/42", "User #42"))
        .child(div().h_px().bg(rgb(0x3e3e3e)).my_2())
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xcccccc))
                .mb_2()
                .child("Invalid Routes"),
        )
        .child(nav_link(cx, "/invalid", "Not Found #1"))
        .child(nav_link(cx, "/missing", "Not Found #2"))
}

fn nav_link(cx: &mut Context<'_, DemoApp>, path: &str, label: &str) -> Div {
    RouterLink::new(path.to_string())
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .text_sm()
                .child(label.to_string()),
        )
        .active_class(|div| div.bg(rgb(0x094771)).text_color(rgb(0xffffff)))
        .build(cx)
        .text_color(rgb(0xcccccc))
        .hover(|this| this.bg(rgb(0x2a2d2e)))
}

fn home_page() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
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
                .bg(rgb(0x2196f3))
                .shadow_lg()
                .child(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(48.))
                        .child("🏠"),
                ),
        )
        .child(
            div()
                .text_3xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child("Welcome Home"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xcccccc))
                .text_center()
                .max_w(px(500.))
                .line_height(relative(1.6))
                .child("This demo shows RouterLink navigation with proper error handling. Try clicking on invalid routes in the sidebar!"),
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
                        .child(feature_item("✓", "Instant navigation with RouterLink"))
                        .child(feature_item("✓", "Active route highlighting"))
                        .child(feature_item("✓", "Smooth page transitions"))
                        .child(feature_item("✓", "Handle invalid routes gracefully")),
                ),
        )
}

fn about_page() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
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
                .bg(rgb(0x9c27b0))
                .shadow_lg()
                .child(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(48.))
                        .child("ℹ️"),
                ),
        )
        .child(
            div()
                .text_3xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child("About This Demo"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xcccccc))
                .text_center()
                .max_w(px(500.))
                .line_height(relative(1.6))
                .child(
                    "A demonstration of GPUI router with RouterLink components and error handling.",
                ),
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
                                .text_color(rgb(0x9c27b0))
                                .mb_2()
                                .child("Features:"),
                        )
                        .child(feature_item("🔗", "RouterLink for declarative navigation"))
                        .child(feature_item("🎨", "Active link styling and hover effects"))
                        .child(feature_item("✨", "Smooth fade and slide transitions"))
                        .child(feature_item("🔍", "Dynamic route parameters"))
                        .child(feature_item("⚠️", "Graceful error handling for 404s")),
                ),
        )
}

fn user_page(params: &RouteParams) -> impl IntoElement {
    let user_id = params.get("id").cloned().unwrap_or_default();

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
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
                .bg(rgb(0x4caf50))
                .shadow_lg()
                .child(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(48.))
                        .child("👤"),
                ),
        )
        .child(
            div()
                .text_3xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child(format!("User #{user_id}")),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0xcccccc))
                .text_center()
                .child("User profile with dynamic route parameter"),
        )
        .child(
            div()
                .mt_4()
                .p_6()
                .bg(rgb(0x252526))
                .rounded(px(12.))
                .border_1()
                .border_color(rgb(0x3e3e3e))
                .w_full()
                .max_w(px(500.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0x4caf50))
                                .mb_2()
                                .child("Profile Details:"),
                        )
                        .child(profile_row("User ID:", &user_id, rgb(0x4ec9b0)))
                        .child(profile_row("Status:", "Active", rgb(0x6a9955)))
                        .child(profile_row("Role:", "Developer", rgb(0xdcdcaa)))
                        .child(profile_row("Member since:", "2024", rgb(0x888888))),
                ),
        )
}

fn profile_row(label: &str, value: &str, value_color: Rgba) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .items_center()
        .py_2()
        .border_b_1()
        .border_color(rgb(0x3e3e3e))
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x888888))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(value_color)
                .child(value.to_string()),
        )
}

fn feature_item(icon: &str, text: &str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(div().text_base().child(icon.to_string()))
        .child(
            div()
                .text_sm()
                .text_color(rgb(0xcccccc))
                .child(text.to_string()),
        )
}
