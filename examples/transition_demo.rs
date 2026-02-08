//! Interactive demo of route transition animations

#![allow(clippy::needless_pass_by_ref_mut)]

use gpui::prelude::*;
use gpui::{
    div, px, relative, rgb, size, App, AppContext, Application, Bounds, Entity, FontWeight,
    MouseButton, Rgba, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use gpui_navigator::{info_log, init_router, Navigator, Route, RouterOutlet, Transition};

fn main() {
    env_logger::init();
    info_log!("Starting transition demo with logging enabled");

    Application::new().run(|cx: &mut App| {
        // Initialize router with routes
        init_router(cx, |router| {
            router.add_route(
                Route::new("/", |_, _, _| home_page().into_any_element())
                    .name("home")
                    .transition(Transition::None),
            );

            router.add_route(
                Route::new("/fade", |_, _, _| fade_page().into_any_element())
                    .name("fade")
                    .transition(Transition::fade(1000)),
            );

            router.add_route(
                Route::new("/slide-left", |_, _, _| {
                    slide_left_page().into_any_element()
                })
                .name("slide-left")
                .transition(Transition::slide_left(1000)), // 1 секунда
            );

            router.add_route(
                Route::new("/slide-right", |_, _, _| {
                    slide_right_page().into_any_element()
                })
                .name("slide-right")
                .transition(Transition::slide_right(1000)), // 1 секунда
            );

            router.add_route(
                Route::new("/slide-up", |_, _, _| slide_up_page().into_any_element())
                    .name("slide-up")
                    .transition(Transition::slide_up(1000)), // 1 секунда
            );

            router.add_route(
                Route::new("/slide-down", |_, _, _| {
                    slide_down_page().into_any_element()
                })
                .name("slide-down")
                .transition(Transition::slide_down(1000)), // 1 секунда
            );
        });

        // Create and open window
        let bounds = Bounds::centered(None, size(px(900.), px(600.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Route Transition Demo".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(TransitionDemoApp::new),
        )
        .unwrap();

        cx.activate(true);
    });
}

struct TransitionDemoApp {
    outlet: Entity<RouterOutlet>,
}

impl TransitionDemoApp {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            outlet: cx.new(|_| RouterOutlet::new()),
        }
    }
}

impl Render for TransitionDemoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0xf5_f5_f5))
            .child(header())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .child(sidebar(cx))
                    .child(div().flex_1().child(self.outlet.clone())),
            )
    }
}

fn header() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h_16()
        .px_8()
        .bg(rgb(0x21_96_f3))
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xff_ff_ff))
                .child("Route Transition Demo"),
        )
}

fn sidebar(cx: &mut Context<'_, TransitionDemoApp>) -> impl IntoElement {
    let current_path = Navigator::current_path(cx);

    div()
        .flex()
        .flex_col()
        .w_64()
        .bg(rgb(0xff_ff_ff))
        .border_r_1()
        .border_color(rgb(0xe0_e0_e0))
        .p_4()
        .gap_2()
        .child(nav_button(cx, "Home (No Transition)", "/", &current_path))
        .child(nav_button(cx, "Fade", "/fade", &current_path))
        .child(nav_button(cx, "Slide Left", "/slide-left", &current_path))
        .child(nav_button(cx, "Slide Right", "/slide-right", &current_path))
        .child(nav_button(cx, "Slide Up", "/slide-up", &current_path))
        .child(nav_button(cx, "Slide Down", "/slide-down", &current_path))
        .child(div().h_px().bg(rgb(0xe0_e0_e0)).my_4())
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x66_66_66))
                .child("Click buttons to test transitions"),
        )
}

fn nav_button(
    cx: &mut Context<'_, TransitionDemoApp>,
    label: &str,
    path: &str,
    current_path: &str,
) -> impl IntoElement {
    let is_active = current_path == path;
    let path = path.to_string();
    let label_str = label.to_string();

    div()
        .id(SharedString::from(label_str.clone()))
        .flex()
        .items_center()
        .px_4()
        .py_3()
        .rounded_md()
        .cursor_pointer()
        .when(is_active, |this| {
            this.bg(rgb(0x21_96_f3)).text_color(rgb(0xff_ff_ff))
        })
        .when(!is_active, |this| {
            this.bg(rgb(0xf5_f5_f5))
                .text_color(rgb(0x33_33_33))
                .hover(|this| this.bg(rgb(0xe3_f2_fd)))
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_view, _event, _window, cx| {
                Navigator::push(cx, path.clone());
            }),
        )
        .child(label_str)
}

fn home_page() -> impl IntoElement {
    page_container(
        "Home - No Transition".to_string(),
        "This page has no transition animation. Simple page without any animation.".to_string(),
        rgb(0x21_96_f3),
        rgb(0xe3_f2_fd), // Light blue background
    )
}

fn fade_page() -> impl IntoElement {
    page_container(
        "Fade Transition".to_string(),
        "Transition::fade(1000) - Cross-fade: old fades out while new fades in.".to_string(),
        rgb(0x9c_27_b0),
        rgb(0xf3_e5_f5), // Light purple background
    )
}

fn slide_left_page() -> impl IntoElement {
    page_container(
        "Slide Left".to_string(),
        "Transition::slide_left(300) - Page slides from left to right.".to_string(),
        rgb(0xf4_43_36),
        rgb(0xff_eb_ee), // Light red background
    )
}

fn slide_right_page() -> impl IntoElement {
    page_container(
        "Slide Right".to_string(),
        "Transition::slide_right(300) - Page slides from right to left.".to_string(),
        rgb(0xff_98_00),
        rgb(0xff_f3_e0), // Light orange background
    )
}

fn slide_up_page() -> impl IntoElement {
    page_container(
        "Slide Up".to_string(),
        "Transition::slide_up(300) - Page slides from top to bottom.".to_string(),
        rgb(0x4c_af_50),
        rgb(0xe8_f5_e9), // Light green background
    )
}

fn slide_down_page() -> impl IntoElement {
    page_container(
        "Slide Down".to_string(),
        "Transition::slide_down(300) - Page slides from bottom to top.".to_string(),
        rgb(0x00_bc_d4),
        rgb(0xe0_f7_fa), // Light cyan background
    )
}

fn page_container(
    title: String,
    description: String,
    color: Rgba,
    bg_color: Rgba,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(bg_color)
        .p_8()
        .items_center()
        .justify_center()
        .gap_6()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w_48()
                .h_48()
                .rounded_lg()
                .bg(color)
                .shadow_lg()
                .child(
                    div()
                        .text_color(rgb(0xff_ff_ff))
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .child("✨"),
                ),
        )
        .child(
            div()
                .text_3xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x21_21_21))
                .child(title),
        )
        .child(
            div()
                .max_w_96()
                .text_center()
                .text_color(rgb(0x66_66_66))
                .line_height(relative(1.5))
                .child(description),
        )
        .child(
            div()
                .mt_8()
                .px_6()
                .py_4()
                .rounded_md()
                .bg(rgb(0xf5_f5_f5))
                .border_1()
                .border_color(rgb(0xe0_e0_e0))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x66_66_66))
                        .child("Click on the sidebar buttons to test different transitions!"),
                ),
        )
}
