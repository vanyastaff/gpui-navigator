//! Route Guards Demo
//!
//! Demonstrates authentication, role-based, and permission-based guards.
//! Toggle auth state, roles, and permissions to see how guards protect routes.

#![allow(clippy::needless_pass_by_ref_mut)]

use gpui::prelude::*;
use gpui::{
    div, px, rgb, size, App, AppContext, Application, Bounds, Entity, FontWeight, Global,
    MouseButton, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use gpui_navigator::{
    guard_fn, init_router, AuthGuard, NavigationAction, Navigator, PermissionGuard, RoleGuard,
    Route, RouteParams, RouterOutlet, Transition,
};

// ============================================================================
// App State — shared via GPUI Global
// ============================================================================

struct AppState {
    is_authenticated: bool,
    user_role: String,
    permissions: Vec<String>,
    last_blocked: Option<String>,
}

impl Global for AppState {}

impl AppState {
    fn new() -> Self {
        Self {
            is_authenticated: false,
            user_role: "user".to_string(),
            permissions: vec!["users.read".to_string()],
            last_blocked: None,
        }
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        cx.set_global(AppState::new());
        setup_routes(cx);

        let bounds = Bounds::centered(None, size(px(1000.), px(700.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Route Guards Demo".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(GuardDemoApp::new),
        )
        .unwrap();

        cx.activate(true);
    });
}

fn setup_routes(cx: &mut App) {
    init_router(cx, |router| {
        // Public: no guards
        router.add_route(
            Route::new("/", |_, _, _| home_page().into_any_element())
                .name("home")
                .transition(Transition::fade(200)),
        );

        // Login: only accessible when NOT authenticated (guests only)
        router.add_route(
            Route::new("/login", |_, cx, _| login_page(cx).into_any_element())
                .name("login")
                .guard(guard_fn(|cx, _req| {
                    if cx.global::<AppState>().is_authenticated {
                        NavigationAction::redirect("/dashboard")
                    } else {
                        NavigationAction::Continue
                    }
                }))
                .transition(Transition::fade(200)),
        );

        // Dashboard: requires authentication
        router.add_route(
            Route::new("/dashboard", |_, cx, _| {
                dashboard_page(cx).into_any_element()
            })
            .name("dashboard")
            .guard(AuthGuard::new(
                |cx| cx.global::<AppState>().is_authenticated,
                "/login",
            ))
            .transition(Transition::slide_left(300)),
        );

        // Admin: requires authentication + "admin" role
        router.add_route(
            Route::new("/admin", |_, cx, _| admin_page(cx).into_any_element())
                .name("admin")
                .guard(AuthGuard::new(
                    |cx| cx.global::<AppState>().is_authenticated,
                    "/login",
                ))
                .guard(RoleGuard::new(
                    |cx| Some(cx.global::<AppState>().user_role.clone()),
                    "admin",
                    Some("/forbidden"),
                ))
                .transition(Transition::slide_left(300)),
        );

        // Delete user: requires auth + "users.delete" permission
        router.add_route(
            Route::new("/users/:id/delete", |_, cx, params| {
                delete_page(cx, params).into_any_element()
            })
            .name("delete_user")
            .guard(AuthGuard::new(
                |cx| cx.global::<AppState>().is_authenticated,
                "/login",
            ))
            .guard(
                PermissionGuard::new(
                    |cx, perm| {
                        cx.global::<AppState>()
                            .permissions
                            .iter()
                            .any(|p| p == perm)
                    },
                    "users.delete",
                )
                .with_redirect("/forbidden"),
            )
            .transition(Transition::fade(200)),
        );

        // Secret: custom inline guard
        router.add_route(
            Route::new("/secret", |_, _, _| secret_page().into_any_element())
                .name("secret")
                .guard(guard_fn(|cx, _req| {
                    let state = cx.global::<AppState>();
                    if state.is_authenticated && state.user_role == "admin" {
                        NavigationAction::Continue
                    } else {
                        NavigationAction::redirect_with_reason(
                            "/forbidden",
                            "Custom guard: admin-only secret area",
                        )
                    }
                }))
                .transition(Transition::fade(200)),
        );

        // Forbidden: always accessible
        router.add_route(
            Route::new("/forbidden", |_, cx, _| {
                forbidden_page(cx).into_any_element()
            })
            .name("forbidden")
            .transition(Transition::fade(200)),
        );
    });
}

// ============================================================================
// Root App Component
// ============================================================================

struct GuardDemoApp {
    outlet: Entity<RouterOutlet>,
}

impl GuardDemoApp {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            outlet: cx.new(|_| RouterOutlet::new()),
        }
    }
}

impl Render for GuardDemoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let state = cx.global::<AppState>();
        let is_auth = state.is_authenticated;
        let role = state.user_role.clone();
        let has_delete = state.permissions.contains(&"users.delete".to_string());
        let current_path = Navigator::current_path(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e_1e_1e))
            .text_color(rgb(0xff_ff_ff))
            .child(render_header(&current_path, is_auth, &role, has_delete))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .child(self.render_sidebar(cx, is_auth, &role, has_delete, &current_path))
                    .child(div().flex_1().child(self.outlet.clone())),
            )
    }
}

fn render_header(
    current_path: &str,
    is_auth: bool,
    role: &str,
    has_delete: bool,
) -> impl IntoElement {
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
                .child("Route Guards Demo"),
        )
        .child(
            div()
                .flex()
                .gap_4()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x88_88_88))
                        .child(format!("Path: {current_path}")),
                )
                .child(status_badge(is_auth, role, has_delete)),
        )
}

#[derive(Clone, Copy)]
enum ToggleKind {
    Auth,
    Role,
    Permission,
}

impl GuardDemoApp {
    #[allow(clippy::unused_self)]
    fn render_sidebar(
        &self,
        cx: &mut Context<'_, Self>,
        is_auth: bool,
        role: &str,
        has_delete: bool,
        current_path: &str,
    ) -> impl IntoElement {
        div()
            .w(px(260.))
            .bg(rgb(0x25_25_26))
            .border_r_1()
            .border_color(rgb(0x3e_3e_3e))
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(section_label("Controls"))
            .child(toggle_button(cx, "Auth", is_auth, ToggleKind::Auth))
            .child(toggle_button(
                cx,
                &format!("Role: {role}"),
                role == "admin",
                ToggleKind::Role,
            ))
            .child(toggle_button(
                cx,
                "Perm: users.delete",
                has_delete,
                ToggleKind::Permission,
            ))
            .child(div().h_px().bg(rgb(0x3e_3e_3e)).my_2())
            .child(section_label("Navigation"))
            .child(nav_button(cx, "/", "Home (public)", current_path))
            .child(nav_button(
                cx,
                "/login",
                "Login (guests only)",
                current_path,
            ))
            .child(nav_button(
                cx,
                "/dashboard",
                "Dashboard (auth)",
                current_path,
            ))
            .child(nav_button(cx, "/admin", "Admin (admin role)", current_path))
            .child(nav_button(
                cx,
                "/users/42/delete",
                "Delete #42 (perm)",
                current_path,
            ))
            .child(nav_button(cx, "/secret", "Secret (custom)", current_path))
            .child(nav_button(
                cx,
                "/forbidden",
                "Forbidden (public)",
                current_path,
            ))
    }
}

fn section_label(text: &str) -> impl IntoElement {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xcc_cc_cc))
        .mb_1()
        .child(text.to_string())
}

fn toggle_button(
    cx: &mut Context<'_, GuardDemoApp>,
    label: &str,
    is_on: bool,
    kind: ToggleKind,
) -> impl IntoElement {
    let label = label.to_string();

    div()
        .id(SharedString::from(format!("toggle-{label}")))
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(if is_on {
            rgb(0x1b_5e_20)
        } else {
            rgb(0x3e_3e_3e)
        })
        .cursor_pointer()
        .hover(|this| {
            this.bg(if is_on {
                rgb(0x2e_7d32)
            } else {
                rgb(0x4e_4e_4e)
            })
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_view, _event, _window, cx| match kind {
                ToggleKind::Auth => {
                    cx.update_global::<AppState, _>(|state, _| {
                        state.is_authenticated = !state.is_authenticated;
                    });
                }
                ToggleKind::Role => {
                    cx.update_global::<AppState, _>(|state, _| {
                        state.user_role = if state.user_role == "admin" {
                            "user".to_string()
                        } else {
                            "admin".to_string()
                        };
                    });
                }
                ToggleKind::Permission => {
                    cx.update_global::<AppState, _>(|state, _| {
                        if state.permissions.contains(&"users.delete".to_string()) {
                            state.permissions.retain(|p| p != "users.delete");
                        } else {
                            state.permissions.push("users.delete".to_string());
                        }
                    });
                }
            }),
        )
        .child(div().text_sm().child(label))
        .child(
            div()
                .text_xs()
                .text_color(if is_on {
                    rgb(0x81_c784)
                } else {
                    rgb(0x88_88_88)
                })
                .child(if is_on { "ON" } else { "OFF" }),
        )
}

fn nav_button(
    cx: &mut Context<'_, GuardDemoApp>,
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

// ============================================================================
// Status Badge
// ============================================================================

fn status_badge(is_auth: bool, role: &str, has_delete: bool) -> impl IntoElement {
    div()
        .flex()
        .gap_2()
        .child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .bg(if is_auth {
                    rgb(0x1b_5e_20)
                } else {
                    rgb(0x4a_14_14)
                })
                .child(if is_auth { "Logged In" } else { "Guest" }),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .bg(rgb(0x33_33_33))
                .child(role.to_string()),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .bg(if has_delete {
                    rgb(0x0d_47_a1)
                } else {
                    rgb(0x33_33_33)
                })
                .child(if has_delete { "delete" } else { "no-delete" }),
        )
}

// ============================================================================
// Pages
// ============================================================================

fn home_page() -> impl IntoElement {
    page_layout(
        "Home",
        "This is a public page with no guards.",
        rgb(0x21_96_f3),
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child("Use the controls in the sidebar to toggle:")
            .child("  - Authentication (logged in / guest)")
            .child("  - Role (user / admin)")
            .child("  - Permission (users.delete)")
            .child("")
            .child("Then try navigating to protected routes to see guards in action."),
    )
}

fn login_page(cx: &App) -> impl IntoElement {
    let is_auth = cx.global::<AppState>().is_authenticated;
    page_layout(
        "Login",
        if is_auth {
            "You are already logged in. This page is protected by NotGuard(AuthGuard)."
        } else {
            "Welcome! Use the Auth toggle in the sidebar to log in."
        },
        rgb(0xff_98_00),
        div(),
    )
}

fn dashboard_page(cx: &App) -> impl IntoElement {
    let state = cx.global::<AppState>();
    page_layout(
        "Dashboard",
        "Protected by AuthGuard. Only authenticated users can see this.",
        rgb(0x4c_af_50),
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(format!("Role: {}", state.user_role))
            .child(format!("Permissions: {:?}", state.permissions)),
    )
}

fn admin_page(cx: &App) -> impl IntoElement {
    let state = cx.global::<AppState>();
    page_layout(
        "Admin Panel",
        "Protected by AuthGuard + RoleGuard(\"admin\").",
        rgb(0x9c_27_b0),
        div().child(format!("Welcome, {} admin!", state.user_role)),
    )
}

fn delete_page(cx: &App, params: &RouteParams) -> impl IntoElement {
    let user_id = params.get("id").cloned().unwrap_or_default();
    let _ = cx.global::<AppState>();
    page_layout(
        &format!("Delete User #{user_id}"),
        "Protected by AuthGuard + PermissionGuard(\"users.delete\").",
        rgb(0xf4_43_36),
        div().child(format!("Confirm deletion of user #{user_id}?")),
    )
}

fn secret_page() -> impl IntoElement {
    page_layout(
        "Secret Area",
        "Protected by a custom guard_fn closure. Requires admin role.",
        rgb(0x00_bc_d4),
        div().child("You found the secret area!"),
    )
}

fn forbidden_page(cx: &App) -> impl IntoElement {
    let last_blocked = cx.global::<AppState>().last_blocked.clone();
    page_layout(
        "Access Denied",
        "You don't have permission to access the requested page.",
        rgb(0x79_55_48),
        div()
            .flex()
            .flex_col()
            .gap_2()
            .when_some(last_blocked, |this, reason| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(rgb(0xff_98_00))
                        .child(format!("Reason: {reason}")),
                )
            })
            .child("Adjust your auth state, role, or permissions and try again."),
    )
}

// ============================================================================
// Shared Layout
// ============================================================================

fn page_layout(
    title: &str,
    description: &str,
    accent: gpui::Rgba,
    extra: impl IntoElement,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .p_8()
        .gap_6()
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
                .mt_2()
                .p_4()
                .bg(rgb(0x2d_2d_2d))
                .rounded_md()
                .border_1()
                .border_color(rgb(0x3e_3e_3e))
                .child(extra),
        )
}
