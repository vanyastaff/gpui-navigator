//! Rendering tests: draw `RouterView`, `RouterOutlet` and `RouterLink` in a
//! real (test) window and check what each navigation renders.
//!
//! These exercise the GPUI runtime APIs the widgets rely on (window drawing,
//! keyed state, animations, mouse events), so they are run against every
//! supported backend in CI.

// Alias the selected GPUI backend to `gpui`, as the library does.
#[cfg(all(
    feature = "gpui-fast",
    not(feature = "gpui-02"),
    not(feature = "gpui-pre")
))]
extern crate gpui_fast as gpui;
#[cfg(all(feature = "gpui-pre", not(feature = "gpui-02")))]
extern crate gpui_pre as gpui;

use gpui::{
    div, px, size, Context, InteractiveElement, IntoElement, Modifiers, ParentElement, Render,
    Styled, TestAppContext, VisualTestContext, Window,
};
use gpui_navigator::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

type Log = Arc<Mutex<Vec<String>>>;

/// Root view that renders the top-level route.
struct Shell;

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div().size_full().child(router_view(window, cx))
    }
}

/// Root view with a `RouterLink` above the routed content.
struct LinkShell;

impl Render for LinkShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                RouterLink::new("/about")
                    .child("About")
                    .build(cx)
                    .w(px(100.))
                    .h(px(20.))
                    .debug_selector(|| "about-link".to_string()),
            )
            .child(router_view(window, cx))
    }
}

/// A route whose builder records `label` (plus any `:id` param) on every render.
fn logged(path: &str, label: &'static str, log: &Log) -> Route {
    let log = log.clone();
    Route::new(path, move |_, _, params| {
        let entry = params
            .get("id")
            .map_or_else(|| label.to_string(), |id| format!("{label}:{id}"));
        log.lock().unwrap().push(entry);
        div().child(label).into_any_element()
    })
}

fn last(log: &Log) -> Option<String> {
    log.lock().unwrap().last().cloned()
}

fn navigate(cx: &mut VisualTestContext, path: &str) {
    cx.update(|_, cx| Navigator::push(cx, path));
    cx.run_until_parked();
}

#[gpui::test]
fn test_router_view_renders_current_route(cx: &mut TestAppContext) {
    let log: Log = Arc::default();
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(logged("/", "home", &log));
            router.add_route(logged("/users/:id", "user", &log));
        });
    });

    let (_, cx) = cx.add_window_view(|_, _| Shell);
    assert_eq!(last(&log).as_deref(), Some("home"));

    navigate(cx, "/users/42");
    assert_eq!(last(&log).as_deref(), Some("user:42"));

    cx.update(|_, cx| Navigator::pop(cx));
    cx.run_until_parked();
    assert_eq!(last(&log).as_deref(), Some("home"));
}

#[gpui::test]
fn test_router_view_renders_not_found(cx: &mut TestAppContext) {
    let log: Log = Arc::default();
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(logged("/", "home", &log));
        });
    });

    let (_, cx) = cx.add_window_view(|_, _| Shell);
    let renders = log.lock().unwrap().len();

    // Drawing the built-in 404 page must not panic and must not render "/".
    navigate(cx, "/missing");
    assert_eq!(log.lock().unwrap().len(), renders);
}

#[gpui::test]
fn test_nested_routes_render_through_outlet(cx: &mut TestAppContext) {
    let log: Log = Arc::default();
    let layout_log = log.clone();
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(logged("/", "home", &log));
            router.add_route(
                Route::new("/dashboard", move |window, cx, _| {
                    layout_log.lock().unwrap().push("layout".to_string());
                    div()
                        .child("Dashboard")
                        .child(render_router_outlet(window, cx, None))
                        .into_any_element()
                })
                .children(vec![
                    Arc::new(logged("overview", "overview", &log)),
                    Arc::new(logged("settings", "settings", &log)),
                ]),
            );
        });
    });

    let (_, cx) = cx.add_window_view(|_, _| Shell);

    navigate(cx, "/dashboard/settings");
    let entries = log.lock().unwrap().clone();
    let layout = entries.iter().rposition(|e| e == "layout");
    let child = entries.iter().rposition(|e| e == "settings");
    assert!(layout.is_some(), "layout not rendered: {entries:?}");
    assert!(
        child > layout,
        "child not rendered after layout: {entries:?}"
    );

    navigate(cx, "/dashboard/overview");
    assert_eq!(last(&log).as_deref(), Some("overview"));
}

#[gpui::test]
fn test_transitions_render_until_finished(cx: &mut TestAppContext) {
    let log: Log = Arc::default();
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(logged("/", "home", &log).transition(Transition::fade(100)));
            router.add_route(logged("/a", "a", &log).transition(Transition::slide_left(100)));
            router.add_route(logged("/b", "b", &log).transition(Transition::slide_up(100)));
        });
    });

    let (_, cx) = cx.add_window_view(|_, _| Shell);

    for (path, label) in [("/a", "a"), ("/b", "b"), ("/", "home")] {
        navigate(cx, path);
        // Let the animation run to completion; every frame must draw cleanly.
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(500));
        cx.run_until_parked();
        assert_eq!(last(&log).as_deref(), Some(label));
    }
}

#[gpui::test]
fn test_component_route_keeps_state_across_frames(cx: &mut TestAppContext) {
    struct Page;
    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
            div().child("page")
        }
    }

    let created = Arc::new(AtomicUsize::new(0));
    let counter = created.clone();
    cx.update(|cx| {
        init_router(cx, move |router| {
            router.add_route(Route::new("/", |_, _, _| div().into_any_element()));
            router.add_route(Route::component_with_context("/form", move |_, cx| {
                counter.fetch_add(1, Ordering::SeqCst);
                let _focus = cx.focus_handle();
                Page
            }));
        });
    });

    let (_, cx) = cx.add_window_view(|_, _| Shell);

    navigate(cx, "/form");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    navigate(cx, "/");
    navigate(cx, "/form");

    assert_eq!(created.load(Ordering::SeqCst), 1);
}

#[gpui::test]
fn test_router_link_click_navigates(cx: &mut TestAppContext) {
    let log: Log = Arc::default();
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(logged("/", "home", &log));
            router.add_route(logged("/about", "about", &log));
        });
    });

    let (_, cx) = cx.add_window_view(|_, _| LinkShell);
    cx.simulate_resize(size(px(800.), px(600.)));
    cx.run_until_parked();

    let bounds = cx
        .debug_bounds("about-link")
        .expect("RouterLink was not drawn");
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();

    assert_eq!(cx.update(|_, cx| Navigator::current_path(cx)), "/about");
    assert_eq!(last(&log).as_deref(), Some("about"));
}
