//! Stateful Route Demo
//!
//! Demonstrates how to create stateful page components using `Route::component()`.
//! This is the CORRECT way to build pages in GPUI - using Entity-based components
//! that maintain state across navigation.

use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Context, Entity, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, Styled, TitlebarOptions, Window, WindowBounds,
    WindowOptions,
};
use gpui_navigator::{init_router, Navigator, Route, RouterOutlet, Transition};

fn main() {
    Application::new().run(|cx: &mut App| {
        // Initialize router with routes using the new ergonomic API
        init_router(cx, |router| {
            router.add_route(
                Route::component("/", HomePage::new)
                    .name("home")
                    .transition(Transition::None),
            );

            router.add_route(
                Route::component("/counter", CounterPage::new)
                    .name("counter")
                    .transition(Transition::fade(300)),
            );

            router.add_route(
                Route::component("/form", FormPage::new)
                    .name("form")
                    .transition(Transition::slide_left(400)),
            );
        });

        // Create and open window
        let bounds = Bounds::centered(None, size(px(800.), px(600.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Stateful Route Demo".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(StatefulDemoApp::new),
        )
        .unwrap();

        cx.activate(true);
    });
}

struct StatefulDemoApp {
    outlet: Entity<RouterOutlet>,
}

impl StatefulDemoApp {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            outlet: cx.new(|_| RouterOutlet::new()),
        }
    }
}

impl Render for StatefulDemoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xffffff))
            // Navigation bar
            .child(
                div()
                    .flex()
                    .gap_2()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .border_b_1()
                    .border_color(rgb(0x3e3e3e))
                    .child(self.nav_button(cx, "/", "Home"))
                    .child(self.nav_button(cx, "/counter", "Counter"))
                    .child(self.nav_button(cx, "/form", "Form")),
            )
            // Router outlet
            .child(div().flex_1().p_4().child(self.outlet.clone()))
    }
}

impl StatefulDemoApp {
    #[allow(clippy::unused_self)]
    fn nav_button(&self, cx: &mut Context<'_, Self>, path: &str, label: &str) -> impl IntoElement {
        let path = path.to_string();
        let label = label.to_string();

        div()
            .px_4()
            .py_2()
            .bg(rgb(0x404040))
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x505050)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_view, _event, _window, cx| {
                    Navigator::push(cx, path.clone());
                }),
            )
            .child(label)
    }
}

// ============================================================================
// Home Page - Simple static content
// ============================================================================

struct HomePage;

impl HomePage {
    const fn new() -> Self {
        Self
    }
}

impl Render for HomePage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_3xl().child("Stateful Route Demo"))
            .child(div().text_lg().child("Pages as Entity-based Components"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child("This demo shows the CORRECT way to build pages in GPUI:")
                    .child("✅ Use Route::component() for stateful pages")
                    .child("✅ Pages are structs that implement Render trait")
                    .child("✅ Entity is automatically cached and reused on navigation")
                    .child("")
                    .child("Navigate to see stateful examples:")
                    .child("• Counter - Entity with internal state")
                    .child("• Form - Entity with form state"),
            )
            .child(
                div()
                    .mt_4()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .text_sm()
                    .child("💡 New Ergonomic API")
                    .child(
                        div()
                            .mt_2()
                            .child("Route::view() - for stateless pages (functions)"),
                    )
                    .child(div().child("Route::component() - for stateful pages (Entity + Render)"))
                    .child(
                        div().child("Route::component_with_params() - for pages with route params"),
                    )
                    .child(
                        div()
                            .mt_2()
                            .child("Route::component() uses window.use_keyed_state() internally!"),
                    ),
            )
    }
}

// ============================================================================
// Counter Page - Demonstrates internal state
// ============================================================================

struct CounterPage {
    count: i32,
}

impl CounterPage {
    const fn new() -> Self {
        Self { count: 0 }
    }
}

impl Render for CounterPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_2xl().child("Counter Page"))
            .child(
                div()
                    .text_lg()
                    .child("Demonstrates Entity with internal state"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(self.button(cx, "-", |page, _cx| {
                        page.count -= 1;
                    }))
                    .child(div().text_2xl().child(format!("{}", self.count)))
                    .child(self.button(cx, "+", |page, _cx| {
                        page.count += 1;
                    }))
                    .child(self.button(cx, "Reset", |page, _cx| {
                        page.count = 0;
                    })),
            )
            .child(
                div()
                    .mt_4()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .text_sm()
                    .child("💡 Counter state persists when you navigate away and back!")
                    .child(
                        div()
                            .mt_2()
                            .child("Try: increment counter, go to Form, return to Counter"),
                    )
                    .child(div().child(
                        "The count is preserved because Route::component() caches the Entity!",
                    )),
            )
    }
}

impl CounterPage {
    #[allow(clippy::unused_self)]
    fn button<F>(&self, cx: &mut Context<'_, Self>, label: &str, on_click: F) -> impl IntoElement
    where
        F: Fn(&mut Self, &mut Context<'_, Self>) + 'static,
    {
        let label = label.to_string();

        div()
            .px_6()
            .py_3()
            .bg(rgb(0x0066cc))
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x0077dd)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |page, _event, _window, cx| {
                    on_click(page, cx);
                    cx.notify();
                }),
            )
            .child(label)
    }
}

// ============================================================================
// Form Page - Demonstrates form state
// ============================================================================

struct FormPage {
    name: String,
    email: String,
    submitted: bool,
}

impl FormPage {
    fn new() -> Self {
        Self {
            name: String::from("John Doe"),
            email: String::from("john@example.com"),
            submitted: false,
        }
    }
}

impl Render for FormPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_2xl().child("Form Page"))
            .child(div().text_lg().child("Demonstrates Entity with form state"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w(px(400.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_sm().child("Name:"))
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .bg(rgb(0x2d2d2d))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(0x404040))
                                    .child(self.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_sm().child("Email:"))
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .bg(rgb(0x2d2d2d))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(0x404040))
                                    .child(self.email.clone()),
                            ),
                    )
                    .child(self.button(cx, "Submit", |page, _cx| {
                        page.submitted = true;
                    }))
                    .child(if self.submitted {
                        div()
                            .mt_4()
                            .p_4()
                            .bg(rgb(0x1a4d1a))
                            .rounded_md()
                            .child("✓ Form submitted!")
                            .child(div().mt_2().text_sm().child(format!("Name: {}", self.name)))
                            .child(div().text_sm().child(format!("Email: {}", self.email)))
                    } else {
                        div()
                    }),
            )
            .child(
                div()
                    .mt_4()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .text_sm()
                    .child("💡 Form state persists across navigation!")
                    .child(
                        div()
                            .mt_2()
                            .child("Submit the form, navigate away, then come back"),
                    )
                    .child(div().child("The submitted state is preserved!")),
            )
    }
}

impl FormPage {
    #[allow(clippy::unused_self)]
    fn button<F>(&self, cx: &mut Context<'_, Self>, label: &str, on_click: F) -> impl IntoElement
    where
        F: Fn(&mut Self, &mut Context<'_, Self>) + 'static,
    {
        let label = label.to_string();

        div()
            .px_6()
            .py_3()
            .bg(rgb(0x0066cc))
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x0077dd)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |page, _event, _window, cx| {
                    on_click(page, cx);
                    cx.notify();
                }),
            )
            .child(label)
    }
}
