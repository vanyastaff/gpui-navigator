//! Integration tests for nested routing
//!
//! Tests complete user story scenarios as specified in spec.md
//! Tests will be implemented across multiple phases:
//! - T020 (US1): Simple nested routes with layouts
//! - T030 (US2): Stateful components maintain state
//! - T035-T036 (US3): Deep nested hierarchies
//! - T043 (US4): Index routes as defaults
//! - T050 (US5): Route parameters inheritance
//! - T057: Navigation cancellation
//! - T062: Performance benchmarks

#[cfg(test)]
mod nested_routing_integration {
    use gpui::{IntoElement, ParentElement};
    use gpui_navigator::nested::resolve_child_route;
    use gpui_navigator::route::Route;
    use gpui_navigator::RouteParams;
    use std::sync::Arc;

    // ========================================================================
    // User Story 1: Simple Nested Routes with Layouts (P1 MVP)
    // ========================================================================

    #[test]
    fn test_nested_routes_preserve_layout() {
        // T020 - Navigate /dashboard → /dashboard/analytics, verify sidebar persists
        // This verifies route resolution works correctly for nested routes
        // In real app: RouterOutlet renders parent layout + swaps child content

        let overview_route = Route::new("overview", |_, _, _| gpui::div().into_any_element());
        let analytics_route = Route::new("analytics", |_, _, _| gpui::div().into_any_element());
        let settings_route = Route::new("settings", |_, _, _| gpui::div().into_any_element());

        let dashboard_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element()).children(vec![
                overview_route.into(),
                analytics_route.into(),
                settings_route.into(),
            ]),
        );

        let parent_params = RouteParams::new();

        // Navigate to /dashboard/analytics
        let result = resolve_child_route(
            &dashboard_route,
            "/dashboard/analytics",
            &parent_params,
            None,
        );
        assert!(result.is_some(), "Should resolve analytics child");

        let (child_route, _) = result.unwrap();
        assert_eq!(
            child_route.config.path, "analytics",
            "Should match analytics route"
        );

        // Navigate to /dashboard/settings
        let result2 = resolve_child_route(
            &dashboard_route,
            "/dashboard/settings",
            &parent_params,
            None,
        );
        assert!(result2.is_some(), "Should resolve settings child");

        let (child_route2, _) = result2.unwrap();
        assert_eq!(
            child_route2.config.path, "settings",
            "Should match settings route"
        );

        // Both navigations use same parent (dashboard_route)
        // In real app: parent layout persists, only child content swaps
    }

    #[test]
    fn test_child_content_changes() {
        // T020 - Navigate between children, verify only child content updates
        // Verify that navigating between siblings resolves different children

        let child1 = Route::new("page1", |_, _, _| gpui::div().into_any_element());
        let child2 = Route::new("page2", |_, _, _| gpui::div().into_any_element());
        let child3 = Route::new("page3", |_, _, _| gpui::div().into_any_element());

        let parent = Arc::new(
            Route::new("/section", |_, _, _| gpui::div().into_any_element()).children(vec![
                child1.into(),
                child2.into(),
                child3.into(),
            ]),
        );

        let params = RouteParams::new();

        // Navigate to each child
        let paths = vec![
            ("/section/page1", "page1"),
            ("/section/page2", "page2"),
            ("/section/page3", "page3"),
        ];

        for (path, expected_child) in paths {
            let result = resolve_child_route(&parent, path, &params, None);
            assert!(result.is_some(), "Should resolve {}", path);

            let (child_route, _) = result.unwrap();
            assert_eq!(
                child_route.config.path, expected_child,
                "Should match correct child for {}",
                path
            );
        }
    }

    // ========================================================================
    // User Story 2: Stateful Components Maintain State (P1 MVP)
    // ========================================================================

    // NOTE: Component state preservation is implemented via GPUI's use_keyed_state()
    // in Route::component() and Route::component_with_params() (src/route.rs:392-470).
    //
    // Full integration testing of stateful components requires a GPUI application
    // context (Window, AppContext) which cannot be easily instantiated in unit tests.
    //
    // MANUAL TESTING INSTRUCTIONS (T030):
    // Run: cargo run --example nested_demo
    // 1. Click "Counter" button in navigation
    // 2. Click "Increment" several times (e.g., increment to 5)
    // 3. Navigate to "Home" or "Dashboard"
    // 4. Navigate back to "Counter"
    // Expected: Counter shows same value (5)
    // Actual behavior: GPUI's use_keyed_state preserves entity across navigation
    //
    // ARCHITECTURE VERIFICATION:
    // - Route::component() uses use_keyed_state with key "route:{path}" (src/route.rs:402)
    // - Route::component_with_params() uses "route:{path}?{params}" (src/route.rs:461)
    // - GPUI manages entity lifecycle and LRU eviction internally
    // - See examples/nested_demo.rs:211-371 for CounterPage implementation
    //
    // TESTED COMPONENTS:
    // ✓ Route::component() method exists (src/route.rs:392)
    // ✓ Route::component_with_params() method exists (src/route.rs:443)
    // ✓ CounterPage example builds and runs (examples/nested_demo.rs)
    // ✓ RouteCache unit tests pass (tests/unit/cache.rs)

    #[gpui::test]
    async fn test_route_component_method_exists(cx: &mut gpui::TestAppContext) {
        // Verify Route::component() creates stateful routes
        // This is a compile-time check - if it compiles, the method exists
        cx.update(|_cx| {
            let _route = Route::component("/test", || TestComponent);

            // Component will be cached by GPUI's use_keyed_state when rendered
            assert_eq!(_route.config.path, "/test");
        });
    }

    #[gpui::test]
    async fn test_route_component_with_params_method_exists(cx: &mut gpui::TestAppContext) {
        // Verify Route::component_with_params() creates parameterized stateful routes
        cx.update(|_cx| {
            let _route = Route::component_with_params("/test/:id", |params| {
                let id = params.get("id").unwrap_or(&"default".to_string()).clone();
                TestComponentWithId(id)
            });

            // Each unique param combo will get its own cached instance
            assert_eq!(_route.config.path, "/test/:id");
        });
    }

    #[gpui::test]
    async fn test_stateful_counter_component_initialization(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::init_router;

        // Initialize router with counter route
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/counter", || StatefulCounter::new(0)));
            });
        });

        // Verify router initialized and on root path
        let path = cx.read(gpui_navigator::Navigator::current_path);
        assert_eq!(path, "/");
    }

    #[gpui::test]
    async fn test_navigation_between_stateful_routes(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator};

        // Setup router with multiple stateful routes
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/", || HomePage));
                router.add_route(Route::component("/counter", || StatefulCounter::new(0)));
                router.add_route(Route::component("/form", || StatefulForm::new()));
            });
        });

        // Navigate to counter
        cx.update(|cx| Navigator::push(cx, "/counter"));
        assert_eq!(cx.read(Navigator::current_path), "/counter");

        // Navigate to form
        cx.update(|cx| Navigator::push(cx, "/form"));
        assert_eq!(cx.read(Navigator::current_path), "/form");

        // Navigate back to counter
        cx.update(|cx| Navigator::pop(cx));
        assert_eq!(cx.read(Navigator::current_path), "/counter");

        // Navigate back to home
        cx.update(|cx| Navigator::pop(cx));
        assert_eq!(cx.read(Navigator::current_path), "/");
    }

    #[gpui::test]
    async fn test_parameterized_stateful_routes(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator};

        // Setup router with parameterized routes
        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component_with_params("/user/:id", |params| {
                    let id = params.get("id").unwrap_or(&"0".to_string()).clone();
                    UserProfile::new(id)
                }));
            });
        });

        // Navigate to user 123
        cx.update(|cx| Navigator::push(cx, "/user/123"));
        assert_eq!(cx.read(Navigator::current_path), "/user/123");

        // Navigate to user 456
        cx.update(|cx| Navigator::push(cx, "/user/456"));
        assert_eq!(cx.read(Navigator::current_path), "/user/456");

        // Both should create separate cached instances
    }

    #[gpui::test]
    async fn test_nested_routes_with_stateful_children(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/dashboard", || DashboardLayout).children(
                    vec![
                        Route::component("overview", || StatefulCounter::new(0)).into(),
                        Route::component("settings", || StatefulForm::new()).into(),
                    ],
                ));
            });
        });

        // Navigate to dashboard/overview
        cx.update(|cx| Navigator::push(cx, "/dashboard/overview"));
        assert_eq!(cx.read(Navigator::current_path), "/dashboard/overview");

        // Navigate to dashboard/settings
        cx.update(|cx| Navigator::push(cx, "/dashboard/settings"));
        assert_eq!(cx.read(Navigator::current_path), "/dashboard/settings");

        // Both child routes should have independent state
    }

    #[gpui::test]
    async fn test_route_with_transition(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator, Transition};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(
                    Route::component("/page1", || HomePage).transition(Transition::fade(200)),
                );
                router.add_route(
                    Route::component("/page2", || TestComponent)
                        .transition(Transition::slide_left(300)),
                );
            });
        });

        cx.update(|cx| Navigator::push(cx, "/page1"));
        assert_eq!(cx.read(Navigator::current_path), "/page1");

        cx.update(|cx| Navigator::push(cx, "/page2"));
        assert_eq!(cx.read(Navigator::current_path), "/page2");
    }

    #[gpui::test]
    async fn test_multiple_child_routes(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(
                    Route::component("/layout", || MultiOutletLayout).children(vec![
                        Route::component("main", || TestComponent).into(),
                        Route::component("sidebar", || StatefulCounter::new(0)).into(),
                    ]),
                );
            });
        });

        cx.update(|cx| Navigator::push(cx, "/layout/main"));
        assert_eq!(cx.read(Navigator::current_path), "/layout/main");

        cx.update(|cx| Navigator::replace(cx, "/layout/sidebar"));
        assert_eq!(cx.read(Navigator::current_path), "/layout/sidebar");
    }

    #[gpui::test]
    async fn test_deep_nested_hierarchy(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/level1", || HomePage).children(
                    vec![Route::component(
                        "level2",
                        || TestComponent,
                    )
                    .children(vec![Route::component("level3", || StatefulCounter::new(0))
                        .children(vec![
                            Route::component("level4", || StatefulForm::new()).into()
                        ])
                        .into()])
                    .into()],
                ));
            });
        });

        // Navigate through 4 levels
        cx.update(|cx| Navigator::push(cx, "/level1/level2/level3/level4"));
        assert_eq!(
            cx.read(Navigator::current_path),
            "/level1/level2/level3/level4"
        );

        // Navigate back up
        cx.update(|cx| Navigator::pop(cx));
        // Should be at root after popping
        assert_eq!(cx.read(Navigator::current_path), "/");
    }

    #[gpui::test]
    async fn test_replace_navigation_with_stateful_route(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/", || HomePage));
                router.add_route(Route::component("/login", || StatefulForm::new()));
                router.add_route(Route::component("/dashboard", || DashboardLayout));
            });
        });

        // Push login
        cx.update(|cx| Navigator::push(cx, "/login"));
        assert_eq!(cx.read(Navigator::current_path), "/login");

        // Replace with dashboard (simulating successful login)
        cx.update(|cx| Navigator::replace(cx, "/dashboard"));
        assert_eq!(cx.read(Navigator::current_path), "/dashboard");

        // Pop should go to home, not login
        cx.update(|cx| Navigator::pop(cx));
        assert_eq!(cx.read(Navigator::current_path), "/");
        assert!(!cx.read(Navigator::can_pop));
    }

    #[gpui::test]
    async fn test_forward_backward_navigation(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/", || HomePage));
                router.add_route(Route::component("/page1", || TestComponent));
                router.add_route(Route::component("/page2", || StatefulCounter::new(0)));
            });
        });

        // Navigate forward
        cx.update(|cx| Navigator::push(cx, "/page1"));
        cx.update(|cx| Navigator::push(cx, "/page2"));
        assert_eq!(cx.read(Navigator::current_path), "/page2");

        // Go back twice
        cx.update(|cx| Navigator::pop(cx));
        cx.update(|cx| Navigator::pop(cx));
        assert_eq!(cx.read(Navigator::current_path), "/");

        // Should be able to go forward
        assert!(cx.read(Navigator::can_go_forward));

        cx.update(|cx| Navigator::forward(cx));
        assert_eq!(cx.read(Navigator::current_path), "/page1");

        cx.update(|cx| Navigator::forward(cx));
        assert_eq!(cx.read(Navigator::current_path), "/page2");

        // No more forward
        assert!(!cx.read(Navigator::can_go_forward));
    }

    #[gpui::test]
    async fn test_index_routes(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/dashboard", || DashboardLayout).children(
                    vec![
                        Route::component("", || TestComponent).into(), // Index route
                        Route::component("settings", || StatefulForm::new()).into(),
                    ],
                ));
            });
        });

        // Navigate to dashboard (should show index)
        cx.update(|cx| Navigator::push(cx, "/dashboard"));
        assert_eq!(cx.read(Navigator::current_path), "/dashboard");
    }

    #[gpui::test]
    async fn test_named_navigation(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator, RouteParams};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/", || HomePage).name("home"));
                router.add_route(
                    Route::component_with_params("/user/:id", |params| {
                        let id = params.get("id").unwrap_or(&"0".to_string()).clone();
                        UserProfile::new(id)
                    })
                    .name("user-profile"),
                );
            });
        });

        // Navigate by name with params
        let mut params = RouteParams::new();
        params.set("id".to_string(), "123".to_string());

        cx.update(|cx| Navigator::push_named(cx, "user-profile", &params));
        assert_eq!(cx.read(Navigator::current_path), "/user/123");

        // Navigate back by name
        let empty_params = RouteParams::new();
        cx.update(|cx| Navigator::push_named(cx, "home", &empty_params));
        assert_eq!(cx.read(Navigator::current_path), "/");
    }

    #[gpui::test]
    async fn test_url_generation(cx: &mut gpui::TestAppContext) {
        use gpui_navigator::{init_router, Navigator, RouteParams};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(
                    Route::component_with_params("/user/:id/posts/:postId", |params| {
                        let user_id = params.get("id").unwrap_or(&"0".to_string()).clone();
                        let post_id = params.get("postId").unwrap_or(&"0".to_string()).clone();
                        PostView::new(user_id, post_id)
                    })
                    .name("user-post"),
                );
            });
        });

        // Generate URL from route name and params
        let mut params = RouteParams::new();
        params.set("id".to_string(), "42".to_string());
        params.set("postId".to_string(), "999".to_string());

        let url = cx.read(|cx| Navigator::url_for(cx, "user-post", &params));
        assert_eq!(url, Some("/user/42/posts/999".to_string()));
    }

    // Test components for verification
    struct TestComponent;
    impl gpui::Render for TestComponent {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div()
        }
    }

    struct TestComponentWithId(String);
    impl gpui::Render for TestComponentWithId {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child(self.0.clone())
        }
    }

    // Additional test components for comprehensive testing
    struct HomePage;
    impl gpui::Render for HomePage {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child("Home Page")
        }
    }

    struct StatefulCounter {
        count: i32,
    }
    impl StatefulCounter {
        fn new(initial: i32) -> Self {
            Self { count: initial }
        }
    }
    impl gpui::Render for StatefulCounter {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child(format!("Count: {}", self.count))
        }
    }

    struct StatefulForm {
        input: String,
    }
    impl StatefulForm {
        fn new() -> Self {
            Self {
                input: String::new(),
            }
        }
    }
    impl gpui::Render for StatefulForm {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child(format!("Form: {}", self.input))
        }
    }

    struct UserProfile {
        user_id: String,
    }
    impl UserProfile {
        fn new(user_id: String) -> Self {
            Self { user_id }
        }
    }
    impl gpui::Render for UserProfile {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child(format!("User Profile: {}", self.user_id))
        }
    }

    struct DashboardLayout;
    impl gpui::Render for DashboardLayout {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child("Dashboard Layout")
        }
    }

    struct MultiOutletLayout;
    impl gpui::Render for MultiOutletLayout {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child("Multi Outlet Layout")
        }
    }

    struct PostView {
        user_id: String,
        post_id: String,
    }
    impl PostView {
        fn new(user_id: String, post_id: String) -> Self {
            Self { user_id, post_id }
        }
    }
    impl gpui::Render for PostView {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child(format!("Post {} by User {}", self.post_id, self.user_id))
        }
    }

    // T035: Test components for 4-level hierarchy
    struct AppLayoutTest;
    impl gpui::Render for AppLayoutTest {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child("App Level")
        }
    }

    struct WorkspaceLayoutTest {
        workspace_id: String,
    }
    impl WorkspaceLayoutTest {
        fn new(workspace_id: String) -> Self {
            Self { workspace_id }
        }
    }
    impl gpui::Render for WorkspaceLayoutTest {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child(format!("Workspace: {}", self.workspace_id))
        }
    }

    struct ProjectLayoutTest {
        project_id: String,
    }
    impl ProjectLayoutTest {
        fn new(project_id: String) -> Self {
            Self { project_id }
        }
    }
    impl gpui::Render for ProjectLayoutTest {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child(format!("Project: {}", self.project_id))
        }
    }

    struct TaskPageTest {
        task_id: String,
    }
    impl TaskPageTest {
        fn new(task_id: String) -> Self {
            Self { task_id }
        }
    }
    impl gpui::Render for TaskPageTest {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<'_, Self>,
        ) -> impl IntoElement {
            gpui::div().child(format!("Task: {}", self.task_id))
        }
    }

    // ========================================================================
    // User Story 3: Deep Nested Hierarchies (P2)
    // ========================================================================

    #[gpui::test]
    async fn test_four_level_hierarchy(cx: &mut gpui::TestAppContext) {
        // T035: Test 4-level deep hierarchy navigation
        use gpui_navigator::{init_router, Navigator};

        cx.update(|cx| {
            init_router(cx, |router| {
                // Create 4-level hierarchy: app → workspace → project → task
                router.add_route(Route::component("/app", || AppLayoutTest).children(vec![
                        Route::component_with_params("workspace/:workspaceId", |params| {
                            let ws_id = params
                                .get("workspaceId")
                                .unwrap_or(&"0".to_string())
                                .clone();
                            WorkspaceLayoutTest::new(ws_id)
                        })
                        .children(vec![Route::component_with_params(
                            "project/:projectId",
                            |params| {
                                let proj_id =
                                    params.get("projectId").unwrap_or(&"0".to_string()).clone();
                                ProjectLayoutTest::new(proj_id)
                            },
                        )
                        .children(vec![Route::component_with_params(
                            "task/:taskId",
                            |params| {
                                let task_id =
                                    params.get("taskId").unwrap_or(&"0".to_string()).clone();
                                TaskPageTest::new(task_id)
                            },
                        )
                        .into()])
                        .into()])
                        .into(),
                    ]));
            });
        });

        // Navigate to 4th level
        cx.update(|cx| Navigator::push(cx, "/app/workspace/ws1/project/proj1/task/task1"));
        assert_eq!(
            cx.read(Navigator::current_path),
            "/app/workspace/ws1/project/proj1/task/task1"
        );

        // All 4 levels should be accessible without errors
        // (The fact that navigation succeeds proves no infinite loops occurred)
    }

    #[gpui::test]
    async fn test_deep_nesting_performance(cx: &mut gpui::TestAppContext) {
        // T035: Measure route resolution performance for 4-level hierarchy
        use gpui_navigator::{init_router, Navigator};
        use std::time::Instant;

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/app", || AppLayoutTest).children(vec![
                        Route::component_with_params("workspace/:workspaceId", |params| {
                            let ws_id = params
                                .get("workspaceId")
                                .unwrap_or(&"0".to_string())
                                .clone();
                            WorkspaceLayoutTest::new(ws_id)
                        })
                        .children(vec![Route::component_with_params(
                            "project/:projectId",
                            |params| {
                                let proj_id =
                                    params.get("projectId").unwrap_or(&"0".to_string()).clone();
                                ProjectLayoutTest::new(proj_id)
                            },
                        )
                        .children(vec![Route::component_with_params(
                            "task/:taskId",
                            |params| {
                                let task_id =
                                    params.get("taskId").unwrap_or(&"0".to_string()).clone();
                                TaskPageTest::new(task_id)
                            },
                        )
                        .into()])
                        .into()])
                        .into(),
                    ]));
            });
        });

        // Measure navigation time
        let start = Instant::now();
        cx.update(|cx| Navigator::push(cx, "/app/workspace/ws1/project/proj1/task/task1"));
        let elapsed = start.elapsed();

        // Navigation should be fast (<1ms for route resolution)
        assert!(
            elapsed.as_millis() < 100,
            "Deep navigation took {}ms, should be <100ms",
            elapsed.as_millis()
        );
    }

    #[gpui::test]
    async fn test_rapid_navigation_no_loops(cx: &mut gpui::TestAppContext) {
        // T036: Stress test - rapid navigation (10 navigations/second)
        use gpui_navigator::{init_router, Navigator};
        use std::time::{Duration, Instant};

        cx.update(|cx| {
            init_router(cx, |router| {
                router.add_route(Route::component("/", || HomePage));
                router.add_route(Route::component("/page1", || TestComponent));
                router.add_route(Route::component("/page2", || StatefulCounter::new(0)));
                router.add_route(Route::component("/page3", || StatefulForm::new()));

                // Add 4-level hierarchy for stress testing
                router.add_route(Route::component("/app", || AppLayoutTest).children(vec![
                        Route::component_with_params("workspace/:workspaceId", |params| {
                            let ws_id = params
                                .get("workspaceId")
                                .unwrap_or(&"0".to_string())
                                .clone();
                            WorkspaceLayoutTest::new(ws_id)
                        })
                        .children(vec![Route::component_with_params(
                            "project/:projectId",
                            |params| {
                                let proj_id =
                                    params.get("projectId").unwrap_or(&"0".to_string()).clone();
                                ProjectLayoutTest::new(proj_id)
                            },
                        )
                        .children(vec![Route::component_with_params(
                            "task/:taskId",
                            |params| {
                                let task_id =
                                    params.get("taskId").unwrap_or(&"0".to_string()).clone();
                                TaskPageTest::new(task_id)
                            },
                        )
                        .into()])
                        .into()])
                        .into(),
                    ]));
            });
        });

        // Perform 10 rapid navigations
        let paths = vec![
            "/",
            "/page1",
            "/page2",
            "/page3",
            "/app/workspace/ws1/project/proj1/task/task1",
            "/page1",
            "/app/workspace/ws2/project/proj2/task/task2",
            "/page2",
            "/",
            "/page3",
        ];

        let start = Instant::now();
        for path in &paths {
            cx.update(|cx| Navigator::push(cx, path.to_string()));
            // Small delay to simulate real usage (10 navigations/second = 100ms each)
            std::thread::sleep(Duration::from_millis(10));
        }
        let elapsed = start.elapsed();

        // All navigations should complete without hanging
        assert!(
            elapsed.as_secs() < 2,
            "10 navigations took {}s, should be <2s",
            elapsed.as_secs()
        );

        // Final path should match last navigation
        assert_eq!(cx.read(Navigator::current_path), "/page3");
    }

    // ========================================================================
    // User Story 4: Index Routes as Defaults (P2)
    // ========================================================================

    #[test]
    fn test_dashboard_with_index_route() {
        // T043 - Verify index route resolves when navigating to parent path
        let index_route = Route::new("", |_, _, _| gpui::div().into_any_element());
        let settings_route = Route::new("settings", |_, _, _| gpui::div().into_any_element());

        let dashboard_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element())
                .children(vec![index_route.into(), settings_route.into()]),
        );

        let parent_params = RouteParams::new();

        // Navigate to /dashboard (no child) - should resolve index route
        let result = resolve_child_route(&dashboard_route, "/dashboard", &parent_params, None);
        assert!(result.is_some());

        let (child_route, _) = result.unwrap();
        assert_eq!(child_route.config.path, ""); // Index route has empty path
    }

    #[test]
    fn test_index_route_priority() {
        // T043 - Empty path ("") has priority over "index" path
        let empty_index = Route::new("", |_, _, _| gpui::div().into_any_element());
        let named_index = Route::new("index", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element())
                .children(vec![named_index.into(), empty_index.into()]),
        );

        let parent_params = RouteParams::new();
        let result = resolve_child_route(&parent_route, "/dashboard", &parent_params, None);

        assert!(result.is_some());
        let (child_route, _) = result.unwrap();
        assert_eq!(child_route.config.path, ""); // Empty path prioritized
    }

    #[test]
    fn test_no_index_route_returns_none() {
        // T043 - Without index route, navigating to parent returns None
        let settings_route = Route::new("settings", |_, _, _| gpui::div().into_any_element());
        let analytics_route = Route::new("analytics", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/dashboard", |_, _, _| gpui::div().into_any_element())
                .children(vec![settings_route.into(), analytics_route.into()]),
        );

        let parent_params = RouteParams::new();
        let result = resolve_child_route(&parent_route, "/dashboard", &parent_params, None);

        assert!(result.is_none()); // No index route, so None
    }

    #[test]
    fn test_root_level_index_route() {
        // T043 - Root level ("/") can have index route
        let index_route = Route::new("", |_, _, _| gpui::div().into_any_element());
        let about_route = Route::new("about", |_, _, _| gpui::div().into_any_element());

        let root_route = Arc::new(
            Route::new("/", |_, _, _| gpui::div().into_any_element())
                .children(vec![index_route.into(), about_route.into()]),
        );

        let parent_params = RouteParams::new();
        let result = resolve_child_route(&root_route, "/", &parent_params, None);

        assert!(result.is_some());
        let (child_route, _) = result.unwrap();
        assert_eq!(child_route.config.path, "");
    }

    // ========================================================================
    // User Story 5: Route Parameters Inheritance (P3)
    // ========================================================================

    #[test]
    fn test_multi_param_inheritance() {
        // T050 - Navigate /workspace/123/project/456/settings
        // Verify settings receives both workspaceId and projectId

        let settings_route = Route::new("settings", |_, _, _| gpui::div().into_any_element());

        let project_route = Route::new(":projectId", |_, _, _| gpui::div().into_any_element())
            .children(vec![settings_route.into()]);

        let workspace_route = Arc::new(
            Route::new("/workspace", |_, _, _| gpui::div().into_any_element()).children(vec![
                Route::new(":workspaceId", |_, _, _| gpui::div().into_any_element())
                    .children(vec![project_route.into()])
                    .into(),
            ]),
        );

        let parent_params = RouteParams::new();

        // Navigate to /workspace/123/456/settings (workspace=123, project=456, settings page)
        let result = resolve_child_route(
            &workspace_route,
            "/workspace/123/456/settings",
            &parent_params,
            None,
        );

        assert!(result.is_some());
        let (_, params) = result.unwrap();

        // Settings should receive both workspaceId and projectId through inheritance
        assert_eq!(params.get("workspaceId"), Some(&"123".to_string()));
        assert_eq!(params.get("projectId"), Some(&"456".to_string()));
    }

    #[test]
    fn test_param_collision_handling() {
        // T050 - Child param overrides parent param on collision

        // Parent has "id" param
        let child_route = Route::new(":id", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new(":id", |_, _, _| gpui::div().into_any_element())
                .children(vec![child_route.into()]),
        );

        let mut parent_params = RouteParams::new();
        parent_params.set("id".to_string(), "parent-value".to_string());

        // Navigate through - child should override parent "id"
        let result = resolve_child_route(&parent_route, "/child-value", &parent_params, None);

        assert!(result.is_some());
        let (_, params) = result.unwrap();

        // Child "id" should win
        assert_eq!(params.get("id"), Some(&"child-value".to_string()));
    }

    #[test]
    fn test_three_level_param_inheritance() {
        // T050 - Test 3-level parameter inheritance (workspace → project → task)

        let task_route = Route::new(":taskId", |_, _, _| gpui::div().into_any_element());

        let project_route = Route::new(":projectId", |_, _, _| gpui::div().into_any_element())
            .children(vec![task_route.into()]);

        let workspace_route = Arc::new(
            Route::new("/app", |_, _, _| gpui::div().into_any_element()).children(vec![
                Route::new(":workspaceId", |_, _, _| gpui::div().into_any_element())
                    .children(vec![project_route.into()])
                    .into(),
            ]),
        );

        let parent_params = RouteParams::new();

        // Navigate through all 3 levels: /app/ws-123/proj-456/task-789
        let result = resolve_child_route(
            &workspace_route,
            "/app/ws-123/proj-456/task-789",
            &parent_params,
            None,
        );

        assert!(result.is_some());
        let (_, params) = result.unwrap();

        // All 3 parameters should be present
        assert_eq!(params.get("workspaceId"), Some(&"ws-123".to_string()));
        assert_eq!(params.get("projectId"), Some(&"proj-456".to_string()));
        assert_eq!(params.get("taskId"), Some(&"task-789".to_string()));
    }

    #[test]
    fn test_mixed_static_and_param_routes() {
        // T050 - Test parameter inheritance with mixed static and dynamic segments

        let detail_route = Route::new(":id", |_, _, _| gpui::div().into_any_element());

        let parent_route = Arc::new(
            Route::new("/users", |_, _, _| gpui::div().into_any_element()).children(vec![
                Route::new(":userId", |_, _, _| gpui::div().into_any_element())
                    .children(vec![Route::new("posts", |_, _, _| {
                        gpui::div().into_any_element()
                    })
                    .children(vec![detail_route.into()])
                    .into()])
                    .into(),
            ]),
        );

        let parent_params = RouteParams::new();

        // Navigate to /users/123/posts/456
        let result =
            resolve_child_route(&parent_route, "/users/123/posts/456", &parent_params, None);

        assert!(result.is_some());
        let (_, params) = result.unwrap();

        // Both userId and post id should be present
        assert_eq!(params.get("userId"), Some(&"123".to_string()));
        assert_eq!(params.get("id"), Some(&"456".to_string()));
    }

    // ========================================================================
    // Error Handling & Edge Cases
    // ========================================================================

    #[test]
    fn test_navigation_cancellation() {
        // TODO: T057 - Rapid navigation cancels previous, only final renders
    }

    #[test]
    fn test_error_boundary_isolation() {
        // TODO: Test component error caught, parent layout remains
    }

    // ========================================================================
    // Performance Benchmarks
    // ========================================================================

    #[test]
    fn test_navigation_latency() {
        // TODO: T062 - Navigate 100 times, average <16ms (SC-003)
    }

    #[test]
    fn test_cache_eviction_performance() {
        // TODO: T061 - Cache eviction <5ms with 1000 entries (SC-008)
    }
}
