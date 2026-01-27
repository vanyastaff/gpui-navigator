//! Nested Routes Demo
//!
//! Demonstrates nested routing with parent/child relationships.
//! Shows how to create layouts with multiple RouterOutlets for child routes.

use gpui::*;
use gpui_navigator::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        // Initialize router with nested route structure
        init_router(cx, |router| {
            // Root route
            router.add_route(
                Route::component("/", HomePage::new)
                    .name("home")
                    .transition(Transition::fade(200)),
            );

            // Dashboard with nested routes
            router.add_route(
                Route::component("/dashboard", DashboardLayout::new)
                    .name("dashboard")
                    .transition(Transition::slide_left(300))
                    .children(vec![
                        // Default child - empty path redirects to overview
                        Route::component("", OverviewPage::new).into(),
                        Route::component("overview", OverviewPage::new)
                            .name("dashboard.overview")
                            .into(),
                        Route::component("analytics", AnalyticsPage::new)
                            .name("dashboard.analytics")
                            .transition(Transition::fade(200))
                            .into(),
                        Route::component("settings", SettingsPage::new)
                            .name("dashboard.settings")
                            .transition(Transition::slide_right(300))
                            .into(),
                    ]),
            );

            // Products with nested routes and parameters
            router.add_route(
                Route::component("/products", ProductsLayout::new)
                    .name("products")
                    .transition(Transition::slide_left(300))
                    .children(vec![
                        // Default child - empty path shows list
                        Route::component("", ProductListPage::new).into(),
                        Route::component("list", ProductListPage::new)
                            .name("products.list")
                            .into(),
                        Route::component_with_params(":id", |params| {
                            let id = params.get("id").unwrap_or(&"unknown".to_string()).clone();
                            ProductDetailPage::new(id)
                        })
                        .name("products.detail")
                        .transition(Transition::fade(200))
                        .into(),
                    ]),
            );
        });

        // Create and open window
        let bounds = Bounds::centered(None, size(px(1000.), px(700.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Nested Routes Demo".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(NestedDemoApp::new),
        )
        .unwrap();

        cx.activate(true);
    });
}

// ============================================================================
// Root App Component
// ============================================================================

struct NestedDemoApp {
    outlet: Entity<RouterOutlet>,
}

impl NestedDemoApp {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            outlet: cx.new(|_| RouterOutlet::new()),
        }
    }
}

impl Render for NestedDemoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xffffff))
            // Top navigation bar
            .child(
                div()
                    .flex()
                    .gap_2()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .border_b_1()
                    .border_color(rgb(0x3e3e3e))
                    .child(self.nav_button(cx, "/", "Home"))
                    .child(self.nav_button(cx, "/dashboard", "Dashboard"))
                    .child(self.nav_button(cx, "/products", "Products")),
            )
            // Main router outlet
            .child(div().flex_1().child(self.outlet.clone()))
    }
}

impl NestedDemoApp {
    fn nav_button(&self, cx: &mut Context<'_, Self>, path: &str, label: &str) -> impl IntoElement {
        let path = path.to_string();
        let label = label.to_string();
        let outlet = self.outlet.clone();

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
                    outlet.update(cx, |_, cx| cx.notify());
                }),
            )
            .child(label)
    }
}

// ============================================================================
// Home Page
// ============================================================================

struct HomePage;

impl HomePage {
    fn new() -> Self {
        Self
    }
}

impl Render for HomePage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_8()
            .child(div().text_3xl().child("Nested Routes Demo"))
            .child(
                div()
                    .text_lg()
                    .child("Demonstrating Parent/Child Route Relationships"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .mt_4()
                    .child("✅ Dashboard has 3 child routes: overview, analytics, settings")
                    .child("✅ Products has nested list and detail pages")
                    .child("✅ Each parent has its own RouterOutlet for children")
                    .child("✅ Child routes inherit parent transitions"),
            )
            .child(
                div()
                    .mt_4()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .child("💡 Try navigating:")
                    .child(
                        div()
                            .mt_2()
                            .child("• Dashboard → Overview/Analytics/Settings"),
                    )
                    .child(div().child("• Products → List → Click a product")),
            )
    }
}

// ============================================================================
// Dashboard Layout (Parent Route)
// ============================================================================

struct DashboardLayout;

impl DashboardLayout {
    fn new() -> Self {
        Self
    }
}

impl Render for DashboardLayout {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let outlet = cx.new(|_| RouterOutlet::new());

        div()
            .flex()
            .size_full()
            // Sidebar navigation
            .child(
                div()
                    .w(px(200.))
                    .bg(rgb(0x252525))
                    .border_r_1()
                    .border_color(rgb(0x3e3e3e))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_xl().mb_4().child("Dashboard"))
                    .child(self.sidebar_link(cx, "/dashboard/overview", "Overview"))
                    .child(self.sidebar_link(cx, "/dashboard/analytics", "Analytics"))
                    .child(self.sidebar_link(cx, "/dashboard/settings", "Settings")),
            )
            // Child routes render here
            .child(div().flex_1().p_8().child(outlet))
    }
}

impl DashboardLayout {
    fn sidebar_link(
        &self,
        cx: &mut Context<'_, Self>,
        path: &str,
        label: &str,
    ) -> impl IntoElement {
        let path = path.to_string();
        let label = label.to_string();

        div()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(0x2d2d2d))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x3d3d3d)))
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
// Dashboard Child Pages
// ============================================================================

struct OverviewPage;

impl OverviewPage {
    fn new() -> Self {
        Self
    }
}

impl Render for OverviewPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_2xl().child("Overview"))
            .child(div().child("Dashboard overview content"))
            .child(
                div()
                    .mt_4()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .child("This is a child route of /dashboard")
                    .child(div().mt_2().child("Full path: /dashboard/overview")),
            )
    }
}

struct AnalyticsPage;

impl AnalyticsPage {
    fn new() -> Self {
        Self
    }
}

impl Render for AnalyticsPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_2xl().child("Analytics"))
            .child(div().child("Dashboard analytics content"))
            .child(
                div()
                    .mt_4()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .child("This page has a fade transition")
                    .child(div().mt_2().child("Full path: /dashboard/analytics")),
            )
    }
}

struct SettingsPage;

impl SettingsPage {
    fn new() -> Self {
        Self
    }
}

impl Render for SettingsPage {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_2xl().child("Settings"))
            .child(div().child("Dashboard settings content"))
            .child(
                div()
                    .mt_4()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .child("This page has a slide-right transition")
                    .child(div().mt_2().child("Full path: /dashboard/settings")),
            )
    }
}

// ============================================================================
// Products Layout (Parent Route with Parameters)
// ============================================================================

struct ProductsLayout;

impl ProductsLayout {
    fn new() -> Self {
        Self
    }
}

impl Render for ProductsLayout {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let outlet = cx.new(|_| RouterOutlet::new());

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_8()
            .child(div().text_2xl().mb_4().child("Products"))
            // Child routes render here
            .child(div().flex_1().child(outlet))
    }
}

// ============================================================================
// Products Child Pages
// ============================================================================

struct ProductListPage;

impl ProductListPage {
    fn new() -> Self {
        Self
    }
}

impl Render for ProductListPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_xl().child("Product List"))
            .child(div().child("Click a product to see details:"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .mt_4()
                    .child(self.product_link(cx, "1", "Product Alpha"))
                    .child(self.product_link(cx, "2", "Product Beta"))
                    .child(self.product_link(cx, "3", "Product Gamma")),
            )
    }
}

impl ProductListPage {
    fn product_link(&self, cx: &mut Context<'_, Self>, id: &str, name: &str) -> impl IntoElement {
        let path = format!("/products/{}", id);
        let name = name.to_string();

        div()
            .px_4()
            .py_2()
            .bg(rgb(0x2d2d2d))
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x3d3d3d)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_view, _event, _window, cx| {
                    Navigator::push(cx, path.clone());
                }),
            )
            .child(name)
    }
}

struct ProductDetailPage {
    product_id: String,
}

impl ProductDetailPage {
    fn new(product_id: String) -> Self {
        Self { product_id }
    }
}

impl Render for ProductDetailPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_xl()
                    .child(format!("Product Detail: {}", self.product_id)),
            )
            .child(div().child("Product information goes here..."))
            .child(
                div()
                    .mt_4()
                    .p_4()
                    .bg(rgb(0x2d2d2d))
                    .rounded_md()
                    .child("This is a dynamic child route with parameters")
                    .child(
                        div()
                            .mt_2()
                            .child(format!("Full path: /products/{}", self.product_id)),
                    )
                    .child(div().mt_2().child("Route pattern: /products/:id")),
            )
            .child(
                div()
                    .mt_4()
                    .px_4()
                    .py_2()
                    .bg(rgb(0x404040))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0x505050)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_view, _event, _window, cx| {
                            Navigator::push(cx, "/products/list".to_string());
                        }),
                    )
                    .child("← Back to List"),
            )
    }
}
