# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-10-19

### Fixed
- Paths with a query string or fragment (`/users?page=2`, `/doc#intro`) now match their route;
  previously they rendered the 404 page
- `*` wildcard routes work in the match stack used for rendering (they only worked in `Route::matches`)
- `:param<constraint>` segments are enforced instead of being parsed and ignored (and `:id<i32>` no
  longer fails path validation)
- Every resolver (match stack, named outlets, `Route::matches`, `RouteParams::from_path`) now shares
  one matcher, so they agree on what a path matches
- Guards and middleware are taken from the routes that actually match the target path; before, a
  guard on `/:id` also ran when navigating to a sibling like `/about`
- A guard redirect during `replace` replaces instead of pushing an extra history entry
- `on_enter` returning deny rolls the navigation back instead of leaving the app on the denied page
- `on_exit` runs before `before_navigation` middleware, so a denied exit never leaves middleware
  half-run; `can_deactivate`/`on_exit`/`on_enter` run on every level that changes instead of only
  the top-level route (exit/deactivate) or the leaf (enter)
- `push_with_state` attaches the state only to the requested path, not to a redirect target
- `NavigationRequest::params` is filled with the target's parameters
- `url_for` substitutes whole segments (`:id` no longer clobbers `:idx`), returns `None` when a
  parameter is missing, and works for named nested routes (registered with their full path)
- `RouterLink` navigates on mouse release instead of press, and ignores the query string when
  deciding it is active
- `RouterOutlet` no longer updates the global router on every render, which re-triggered
  `observe_global::<GlobalRouter>` observers each frame
- `QueryParams::from_query_string` accepts a leading `?` and bare flags (`?debug`)

### Added
- Async guards: `async_guard_fn`, `RouteGuard::check_async`, `NavigationResult::Pending`,
  `Navigator::is_pending`; a newer navigation cancels a pending one
- `Navigator::try_push` / `try_replace` / `try_back` / `try_forward` / `try_push_named` return the
  `NavigationResult`
- `Navigator::query` and `Navigator::params`, `GlobalRouter::current_query`, `QueryParams::from_location`,
  `QueryParams::iter`, `split_path_and_query`, `path_only`
- Path constraints `int`, `i32`, `uint`, `u32`, `usize`, `float`, `bool`, `uuid`, `alpha`, `alnum`,
  `slug`, and named wildcards `*rest`
- `Route::can_leave::<Page>(|page, cx| ...)` checks the cached page entity before leaving
  (`RouteLifecycle::can_deactivate_page`)
- `Route::keep_alive(false)`; component cache is now LRU with `set_component_cache_capacity`,
  `evict_components`, `clear_component_cache`, `component_cache_len`
- `GoBack` / `GoForward` actions, `bind_navigation_keys`, `register_navigation_actions`,
  `with_mouse_navigation` (mouse side buttons)
- `on_route_change` subscription, `GlobalRouter::last_change` / `change_count`
- `HistorySnapshot`, `GlobalRouter::history_snapshot` / `restore_history`, `RouterState::history`,
  `HistoryState::with` / `get_as`, and a `serde` feature for history types
- Transitions: `Transition::with_easing`, `Transition::custom`, `Transition::reversed`,
  `SlideDirection::opposite`, easing functions `linear`, `ease_in_cubic`, `ease_out_cubic`;
  slides mirror when navigating back (`GlobalRouter::set_reverse_transitions_on_back`)
- `RouterLink::active_match(ActiveMatch::Prefix)` and `is_link_active`
- `router_debug_panel` showing location, match stack, history and cache
- A warning at registration when a route is unreachable because an earlier sibling covers it
- `desktop_demo` example and a standalone `examples/gpui-component` app
- Property tests for path matching, pipeline tests, and a route-resolution benchmark
  (`cargo bench --bench resolve`)
- CI: semver check against the last release, and a build of the gpui-component example

### Changed
- `apply_transition` uses the same styling as `RouterOutlet` (slides move by the container's size
  instead of a fixed 100px and no longer fade)

## [0.2.0] - 2026-10-19

### Breaking
- GPUI is now selected through mutually exclusive backend features: `gpui-02` (default, `gpui` 0.2),
  `gpui-pre` and `gpui-fast` (#2). Builds with `default-features = false` must add a backend
  feature, e.g. `features = ["gpui-02", ...]`.

### Added
- `gpui-pre` and `gpui-fast` backends, for use with gpui-component 0.6.x and 0.7.x (#2)
- `Route::component_with_context` and `Route::component_with_params_and_context`: stateful routes whose
  factory receives `&mut Window` and `&mut Context<T>`, so pages can create child entities such as
  gpui-component's `InputState` (#1)
- README note on wrapping the window root in `gpui_component::Root`
- `test-support` feature, forwarding to the selected backend's `test-support` (used by this crate's own tests)
- Rendering tests that draw `RouterView`, `RouterOutlet`, `RouterLink` and route transitions in a test
  window; CI runs the full test suite on every backend
- Initial public release preparation
- Comprehensive documentation for all public APIs
- LICENSE-MIT and LICENSE-APACHE files
- CHANGELOG.md following Keep a Changelog format
- CI/CD configuration for GitHub Actions

### Changed
- `AuthGuard` now requires a check function instead of using a placeholder
- `RoleGuard` now requires a role extractor function for proper configuration
- `PermissionGuard` now requires a permission check function
- Improved error messages for guard failures
- Updated Cargo.toml with production-ready metadata

### Fixed
- `Route::component_with_params` cache keys no longer depend on `HashMap` iteration order, so routes with
  several parameters reuse their cached entity instead of recreating it
- Tests now build on the `gpui-pre` and `gpui-fast` backends (they previously required `gpui` 0.2)
- CI workflow: use `dtolnay/rust-toolchain`, install `libxkbcommon-x11-dev`, run the MSRV job on the declared MSRV
- MSRV raised to 1.88 to match what the gpui 0.2 dependency tree actually requires
- Guards no longer use hardcoded `false` returns
- Removed dead code warnings in nested route cache
- Fixed clippy warnings throughout the codebase

## [0.1.0] - 2024-01-01

### Added
- **Core Router**
  - `GlobalRouter` for application-wide routing state
  - `RouterState` for managing navigation history
  - `Navigator` with Flutter-style navigation API
  - `init_router` for easy router initialization

- **Route Definition**
  - `Route` builder with fluent API
  - `RouteConfig` for route metadata
  - Named routes with `NamedRouteRegistry`
  - Route parameters with type-safe extraction
  - Wildcard and pattern matching support

- **Transitions**
  - `Transition::fade()` for opacity animations
  - `Transition::slide_left/right/up/down()` for directional slides
  - `Transition::zoom_in/zoom_out()` for scale animations
  - Custom transition support via `TransitionAnimation` trait

- **Nested Routing**
  - `RouterOutlet` component for child route rendering
  - Named outlets for multiple content areas
  - `RouteCache` for optimized route resolution
  - Parent/child route hierarchy support

- **Guards**
  - `AuthGuard` for authentication checks
  - `RoleGuard` for role-based authorization
  - `PermissionGuard` for permission-based access control
  - `Guards` for composing multiple guards (AND logic)
  - `NotGuard` for inverting guard results
  - `guard_fn` helper for creating guards from closures

- **Middleware**
  - `RouteMiddleware` trait for before/after navigation hooks
  - `middleware_fn` helper for creating middleware from functions
  - Priority-based middleware execution order

- **Lifecycle Hooks**
  - `RouteLifecycle` trait for route lifecycle management
  - `on_enter` hook for route initialization
  - `on_exit` hook for cleanup
  - `can_deactivate` hook for navigation confirmation

- **Error Handling**
  - `NavigationError` for typed error handling
  - `NavigationResult` for navigation outcomes
  - `ErrorHandlers` for custom 404 and error pages

- **Parameters**
  - `RouteParams` for path parameters
  - `QueryParams` for query string parsing
  - Type-safe parameter extraction with `get_as<T>()`

- **Widgets**
  - `RouterOutlet` for outlet rendering
  - `RouterLink` for declarative navigation links
  - Helper functions: `router_outlet()`, `router_link()`

- **Advanced Matching**
  - `RoutePattern` for complex route patterns
  - `Segment` types: Static, Param, Optional, Wildcard
  - `Constraint` validation: Numeric, UUID, Pattern

### Documentation
- Comprehensive README with examples
- Doc comments for all public APIs
- Example application: `transition_demo`

[Unreleased]: https://github.com/vanyastaff/gpui-navigator/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/vanyastaff/gpui-navigator/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/vanyastaff/gpui-navigator/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/vanyastaff/gpui-navigator/releases/tag/v0.1.0