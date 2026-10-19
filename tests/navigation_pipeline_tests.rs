//! Navigation pipeline and desktop-input behaviour: path matching (query
//! strings, wildcards, constraints), guards, lifecycle hooks, async guards,
//! page caching, route change events, back/forward actions and transitions.

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
    div, px, size, App, BorrowAppContext, Context, IntoElement, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window,
};
use gpui_navigator::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Log = Arc<Mutex<Vec<String>>>;

struct Shell;

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div().size_full().child(router_view(window, cx))
    }
}

fn page(path: &str) -> Route {
    Route::view(path, || div().into_any_element())
}

fn path(cx: &App) -> String {
    Navigator::current_path(cx)
}

fn leaf_path(cx: &App) -> Option<String> {
    cx.global::<GlobalRouter>()
        .match_stack()
        .leaf()
        .map(|entry| entry.route.config.path.clone())
}

fn window(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let (_, cx) = cx.add_window_view(|_, _| Shell);
    cx.simulate_resize(size(px(800.), px(600.)));
    cx.run_until_parked();
    cx
}

fn go(cx: &mut VisualTestContext, path: &str) {
    cx.update(|_, cx| Navigator::push(cx, path));
    cx.run_until_parked();
}

// ============================================================================
// Matching
// ============================================================================

#[gpui::test]
fn test_query_string_does_not_break_matching(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(page("/users"));
        });
        let result = Navigator::try_push(cx, "/users?page=2&debug");
        assert!(result.is_success());
        assert_eq!(leaf_path(cx).as_deref(), Some("/users"));
        assert_eq!(Navigator::query(cx).get_as::<u32>("page"), Some(2));
        assert!(Navigator::query(cx).contains("debug"));
        assert_eq!(path(cx), "/users?page=2&debug");
    });
}

#[gpui::test]
fn test_wildcard_and_constraints(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(page("/posts/:id<int>"));
            router.add_route(page("/posts/:slug"));
            router.add_route(page("/files/*path"));
            router.add_route(page("*"));
        });

        Navigator::push(cx, "/posts/42");
        assert_eq!(leaf_path(cx).as_deref(), Some("/posts/:id<int>"));
        assert_eq!(Navigator::params(cx).get_as::<i64>("id"), Some(42));

        Navigator::push(cx, "/posts/hello-world");
        assert_eq!(leaf_path(cx).as_deref(), Some("/posts/:slug"));

        Navigator::push(cx, "/files/docs/readme.md");
        assert_eq!(leaf_path(cx).as_deref(), Some("/files/*path"));
        assert_eq!(
            Navigator::params(cx).get("path").map(String::as_str),
            Some("docs/readme.md")
        );

        Navigator::push(cx, "/no/such/page");
        assert_eq!(leaf_path(cx).as_deref(), Some("*"));
    });
}

#[gpui::test]
fn test_named_nested_route_url(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/users").children(vec![Arc::new(
                page(":id").children(vec![Arc::new(page("edit").name("user.edit"))]),
            )]));
        });
        let mut params = RouteParams::new();
        params.insert("id", "7");
        assert_eq!(
            Navigator::url_for(cx, "user.edit", &params).as_deref(),
            Some("/users/7/edit")
        );
        assert!(Navigator::try_push_named(cx, "user.edit", &params).is_some());
        assert_eq!(path(cx), "/users/7/edit");
    });
}

// ============================================================================
// Guards and lifecycle
// ============================================================================

#[gpui::test]
fn test_guards_only_from_matched_routes(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(page("/about"));
            // Matches `/about` by prefix, but `/about` is resolved to the route above.
            router.add_route(page("/:id").guard(guard_fn(|_, _| NavigationAction::deny("no ids"))));
        });
        assert!(Navigator::try_push(cx, "/about").is_success());
        assert!(Navigator::try_push(cx, "/42").is_blocked());
        assert_eq!(path(cx), "/about");
    });
}

#[gpui::test]
fn test_redirect_during_replace_does_not_add_history(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(page("/login"));
            router.add_route(
                page("/admin").guard(guard_fn(|_, _| NavigationAction::redirect("/login"))),
            );
        });
        Navigator::push(cx, "/login");
        Navigator::replace(cx, "/admin");
        assert_eq!(path(cx), "/login");
        let history = cx.global::<GlobalRouter>().history_snapshot();
        assert_eq!(history.entries.len(), 2, "{history:?}");
    });
}

struct Hooks {
    log: Log,
    name: &'static str,
    deny_enter: bool,
}

impl RouteLifecycle for Hooks {
    fn on_enter(&self, _: &App, _: &NavigationRequest) -> NavigationAction {
        self.log
            .lock()
            .unwrap()
            .push(format!("enter {}", self.name));
        if self.deny_enter {
            NavigationAction::deny("not today")
        } else {
            NavigationAction::Continue
        }
    }

    fn on_exit(&self, _: &App) -> NavigationAction {
        self.log.lock().unwrap().push(format!("exit {}", self.name));
        NavigationAction::Continue
    }

    fn can_deactivate(&self, _: &App) -> NavigationAction {
        NavigationAction::Continue
    }
}

fn hooks(log: &Log, name: &'static str, deny_enter: bool) -> Hooks {
    Hooks {
        log: log.clone(),
        name,
        deny_enter,
    }
}

#[gpui::test]
fn test_lifecycle_runs_on_levels_entered_and_left(cx: &mut TestAppContext) {
    let log: Log = Arc::default();
    let routes_log = log.clone();
    cx.update(|cx| {
        init_router(cx, move |router| {
            router.add_route(page("/").lifecycle(hooks(&routes_log, "home", false)));
            router.add_route(
                page("/settings")
                    .lifecycle(hooks(&routes_log, "settings", false))
                    .children(vec![
                        Arc::new(page("profile").lifecycle(hooks(&routes_log, "profile", false))),
                        Arc::new(page("account").lifecycle(hooks(&routes_log, "account", false))),
                    ]),
            );
        });
        Navigator::push(cx, "/settings/profile");
        Navigator::push(cx, "/settings/account");
        Navigator::push(cx, "/");
    });

    let log = log.lock().unwrap().clone();
    assert_eq!(
        log,
        [
            "exit home",
            "enter settings",
            "enter profile",
            // switching children leaves the parent mounted
            "exit profile",
            "enter account",
            // leaving the section: leaf first, then parent
            "exit account",
            "exit settings",
            "enter home",
        ]
    );
}

#[gpui::test]
fn test_on_enter_deny_rolls_back(cx: &mut TestAppContext) {
    let log: Log = Arc::default();
    let routes_log = log.clone();
    cx.update(|cx| {
        init_router(cx, move |router| {
            router.add_route(page("/"));
            router.add_route(page("/closed").lifecycle(hooks(&routes_log, "closed", true)));
        });
        let result = Navigator::try_push(cx, "/closed");
        assert!(result.is_blocked());
        assert_eq!(path(cx), "/");
        assert_eq!(leaf_path(cx).as_deref(), Some("/"));
        assert!(!Navigator::can_go_back(cx));
    });
}

#[gpui::test]
fn test_can_leave_reads_page_state(cx: &mut TestAppContext) {
    struct Editor {
        dirty: bool,
    }
    impl Render for Editor {
        fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
            div().child("editor")
        }
    }

    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(
                Route::component("/editor", || Editor { dirty: true }).can_leave(
                    |editor: &Editor, _| {
                        if editor.dirty {
                            NavigationAction::deny("unsaved changes")
                        } else {
                            NavigationAction::Continue
                        }
                    },
                ),
            );
        });
    });
    let cx = window(cx);
    go(cx, "/editor");
    cx.update(|_, cx| {
        assert!(Navigator::try_push(cx, "/").is_blocked());
        assert_eq!(path(cx), "/editor");
    });
}

// ============================================================================
// Async guards
// ============================================================================

#[gpui::test]
fn test_async_guard_completes_navigation(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(
                page("/ok").guard(async_guard_fn(|_, _| async { NavigationAction::Continue })),
            );
            router.add_route(page("/no").guard(async_guard_fn(|_, _| async {
                NavigationAction::deny("nope")
            })));
            router.add_route(page("/login"));
            router.add_route(page("/away").guard(async_guard_fn(|_, _| async {
                NavigationAction::redirect("/login")
            })));
        });
    });

    let result = cx.update(|cx| Navigator::try_push(cx, "/ok"));
    assert!(result.is_pending());
    cx.update(|cx| {
        assert!(Navigator::is_pending(cx));
        assert_eq!(path(cx), "/");
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!Navigator::is_pending(cx));
        assert_eq!(path(cx), "/ok");
    });

    cx.update(|cx| Navigator::push(cx, "/no"));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(path(cx), "/ok"));

    cx.update(|cx| Navigator::push(cx, "/away"));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(path(cx), "/login"));
}

#[gpui::test]
fn test_newer_navigation_cancels_pending_one(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(page("/fast"));
            router.add_route(page("/slow").guard(async_guard_fn(|cx, _| {
                let timer = cx.background_executor().timer(Duration::from_millis(100));
                async move {
                    timer.await;
                    NavigationAction::Continue
                }
            })));
        });
    });

    cx.update(|cx| Navigator::push(cx, "/slow"));
    cx.update(|cx| Navigator::push(cx, "/fast"));
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(path(cx), "/fast");
        assert!(!Navigator::is_pending(cx));
    });
}

// ============================================================================
// Page cache, events, actions, history
// ============================================================================

#[gpui::test]
fn test_keep_alive_false_rebuilds_page(cx: &mut TestAppContext) {
    struct Form;
    impl Render for Form {
        fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
            div().child("form")
        }
    }

    let created = Arc::new(AtomicUsize::new(0));
    let counter = created.clone();
    cx.update(|cx| {
        init_router(cx, move |router| {
            router.add_route(page("/"));
            router.add_route(
                Route::component("/form", move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Form
                })
                .keep_alive(false),
            );
        });
    });
    let cx = window(cx);
    go(cx, "/form");
    go(cx, "/");
    go(cx, "/form");
    assert_eq!(created.load(Ordering::SeqCst), 2);
}

#[gpui::test]
fn test_component_cache_capacity_and_eviction(cx: &mut TestAppContext) {
    struct User;
    impl Render for User {
        fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
            div().child("user")
        }
    }

    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(Route::component_with_params("/users/:id", |_| User));
            router.set_component_cache_capacity(2);
        });
    });
    let cx = window(cx);
    for id in 1..=3 {
        go(cx, &format!("/users/{id}"));
    }
    cx.update(|_, cx| {
        assert_eq!(cx.global::<GlobalRouter>().component_cache_len(), 2);
    });
    cx.update(|_, cx| {
        cx.update_global::<GlobalRouter, _>(|router, _| {
            assert_eq!(router.evict_components("/users/:id"), 2);
        });
    });
}

#[gpui::test]
fn test_on_route_change_fires_once_per_navigation(cx: &mut TestAppContext) {
    let events: Log = Arc::default();
    let sink = events.clone();
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(page("/a"));
        });
        on_route_change(cx, move |event, _| {
            sink.lock().unwrap().push(event.to.clone());
        })
        .detach();
    });
    let cx = window(cx);
    go(cx, "/a");
    // Re-rendering must not fire the callback again.
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.update(|_, cx| Navigator::back(cx));
    cx.run_until_parked();
    assert_eq!(events.lock().unwrap().clone(), ["/a", "/"]);
}

#[gpui::test]
fn test_back_forward_actions(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(page("/a"));
        });
        register_navigation_actions(cx);
    });
    let cx = window(cx);
    go(cx, "/a");
    cx.dispatch_action(GoBack);
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| path(cx)), "/");
    cx.dispatch_action(GoForward);
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| path(cx)), "/a");
}

#[gpui::test]
fn test_history_snapshot_round_trip(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(page("/a"));
            router.add_route(page("/b"));
        });
        Navigator::push(cx, "/a");
        Navigator::push(cx, "/b");
        Navigator::back(cx);
        let snapshot = cx.global::<GlobalRouter>().history_snapshot();

        cx.set_global(GlobalRouter::new());
        cx.update_global::<GlobalRouter, _>(|router, _| {
            router.add_route(page("/"));
            router.add_route(page("/a"));
            router.add_route(page("/b"));
            assert!(router.restore_history(snapshot));
        });
        assert_eq!(path(cx), "/a");
        assert_eq!(leaf_path(cx).as_deref(), Some("/a"));
        assert!(Navigator::can_go_forward(cx));
    });
}

// ============================================================================
// Transitions
// ============================================================================

#[gpui::test]
fn test_custom_and_eased_transitions_render(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init_router(cx, |router| {
            router.add_route(page("/"));
            router.add_route(
                page("/a").transition(Transition::slide_left(100).with_easing(ease_out_cubic)),
            );
            router.add_route(page("/b").transition(Transition::custom(
                100,
                |layer, p| layer.opacity(p),
                |layer, p| layer.opacity(1.0 - p),
            )));
        });
    });
    let cx = window(cx);
    for path in ["/a", "/b", "/a"] {
        go(cx, path);
        cx.executor().advance_clock(Duration::from_millis(300));
        cx.run_until_parked();
    }
    cx.update(|_, cx| Navigator::back(cx));
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(cx.global::<GlobalRouter>().is_navigating_back());
    });
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
}

#[test]
fn test_slides_reverse_when_going_back() {
    let reversed = Transition::slide_left(200).reversed();
    assert!(matches!(
        reversed,
        Transition::Slide {
            direction: SlideDirection::Right,
            ..
        }
    ));
    let eased = Transition::slide_up(200)
        .with_easing(ease_in_cubic)
        .reversed();
    assert!(matches!(
        eased,
        Transition::Eased {
            kind: EasedKind::Slide(SlideDirection::Down),
            ..
        }
    ));
    assert_eq!(eased.duration(), Duration::from_millis(200));
}
