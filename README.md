# gpui-navigator

[![Crates.io](https://img.shields.io/crates/v/gpui-navigator.svg)](https://crates.io/crates/gpui-navigator)
[![Documentation](https://docs.rs/gpui-navigator/badge.svg)](https://docs.rs/gpui-navigator)
[![License](https://img.shields.io/crates/l/gpui-navigator.svg)](LICENSE-MIT)

Declarative client-side navigation for [GPUI](https://gpui.rs) (Zed's GPU-accelerated UI framework). Provides route matching, nested layouts, animated transitions, guards, middleware, and LRU caching — all behind feature flags so you pay only for what you use.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Defining Routes](#defining-routes)
  - [Route::view — Stateless Pages](#routeview--stateless-pages)
  - [Route::component — Stateful Pages](#routecomponent--stateful-pages)
  - [Route::component_with_params — Stateful + Params](#routecomponent_with_params--stateful--params)
  - [Route::new — Full Control](#routenew--full-control)
- [Navigation API](#navigation-api)
  - [Programmatic Navigation](#programmatic-navigation)
  - [Fluent API](#fluent-api)
  - [Named Routes](#named-routes)
- [Widgets](#widgets)
  - [RouterView](#routerview)
  - [RouterOutlet](#routeroutlet)
  - [RouterLink](#routerlink)
- [Nested Routing](#nested-routing)
  - [Named Outlets](#named-outlets)
  - [Index Routes](#index-routes)
  - [Parameter Inheritance](#parameter-inheritance)
- [Route Parameters](#route-parameters)
  - [Path Parameters](#path-parameters)
  - [Query Parameters](#query-parameters)
- [Transitions](#transitions)
- [Route Guards](#route-guards)
- [Middleware](#middleware)
- [Route Lifecycle](#route-lifecycle)
- [Error Handling](#error-handling)
- [Caching](#caching)
- [Feature Flags](#feature-flags)
- [Examples](#examples)
- [Architecture Overview](#architecture-overview)
- [API Reference](#api-reference)
- [License](#license)

## Features

- **Smooth Transitions** — Fade, slide (4 directions) with configurable duration and dual enter/exit animation
- **Nested Routing** — Unlimited nesting depth with `RouterOutlet`, named outlets, index routes
- **Stateful Components** — `Route::component()` auto-caches GPUI entities across navigations
- **Route Guards** — `AuthGuard`, `RoleGuard`, `PermissionGuard`, composable `NotGuard`
- **Middleware** — Before/after navigation hooks with priority ordering
- **Named Routes** — Navigate by name with parameter substitution
- **Route Lifecycle** — `on_enter`, `on_exit`, `can_deactivate` hooks
- **LRU Cache** — Route resolution caching with hit-rate stats
- **Error Pages** — Built-in styled 404 and error pages, fully customizable
- **RouterLink** — Navigation links with automatic active-state styling
- **Type-safe Params** — `get_as::<T>()` for parsed parameter extraction
- **Logging** — `log` or `tracing` backend (mutually exclusive, feature-gated)

## Installation

```toml
[dependencies]
gpui-navigator = "0.1"
gpui = "0.2"
```

All features are enabled by default. To pick only what you need:

```toml
[dependencies]
gpui-navigator = { version = "0.1", default-features = false, features = ["transition", "guard"] }
```

## Quick Start

```rust
use gpui::prelude::*;
use gpui::*;
use gpui_navigator::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        init_router(cx, |router| {
            router.add_route(
                Route::view("/", || div().child("Home").into_any_element())
                    .transition(Transition::fade(300))
            );
            router.add_route(
                Route::view("/about", || div().child("About").into_any_element())
                    .transition(Transition::slide_left(400))
            );
        });

        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| AppShell)
        }).unwrap();
    });
}

struct AppShell;

impl Render for AppShell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(RouterView::new())
    }
}
```

`init_router` registers routes globally. `RouterView` at the top level renders whichever route matches the current path.

## Defining Routes

### `Route::view` — Stateless Pages

Simplest option. Takes a path and a closure that returns `AnyElement`:

```rust
Route::view("/about", || {
    div()
        .p_4()
        .child("About this app")
        .into_any_element()
})
```

### `Route::component` — Stateful Pages

Wraps a GPUI `Entity` that persists across navigations. State is preserved when the user navigates away and back:

```rust
struct CounterPage { count: i32 }

impl CounterPage {
    fn new() -> Self { Self { count: 0 } }
}

impl Render for CounterPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(format!("Count: {}", self.count))
            .child(
                div()
                    .cursor_pointer()
                    .child("Increment")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.count += 1;
                        cx.notify();
                    }))
            )
    }
}

Route::component("/counter", CounterPage::new)
```

### `Route::component_with_params` — Stateful + Params

Like `component`, but the factory receives `RouteParams`. Each unique parameter set gets its own cached entity:

```rust
struct UserPage { user_id: String }

impl UserPage {
    fn new(id: String) -> Self { Self { user_id: id } }
}

impl Render for UserPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(format!("User: {}", self.user_id))
    }
}

Route::component_with_params("/users/:id", |params| {
    let id = params.get("id").unwrap().clone();
    UserPage::new(id)
})
```

### `Route::new` — Full Control

Receives `&mut Window`, `&mut App`, and `&RouteParams`:

```rust
Route::new("/dashboard", |window, cx, params| {
    // Full access to GPUI context
    dashboard_view(window, cx, params).into_any_element()
})
```

## Navigation API

### Programmatic Navigation

All methods are static on `Navigator`:

```rust
use gpui_navigator::Navigator;

// Push a new route onto the history stack
Navigator::push(cx, "/users/42");

// Replace the current route (no new history entry)
Navigator::replace(cx, "/login");

// Go back / forward in history
Navigator::pop(cx);
Navigator::forward(cx);

// Query state
let path: String = Navigator::current_path(cx);
let can_back: bool = Navigator::can_pop(cx);
let can_fwd: bool = Navigator::can_go_forward(cx);
```

### Fluent API

Chain multiple navigations:

```rust
Navigator::of(cx)
    .push("/step-1")
    .push("/step-2")
    .push("/step-3");
```

### Named Routes

Define routes with names, navigate by name with parameter substitution:

```rust
// Define
Route::new("/users/:id/posts/:post_id", handler)
    .name("user-post")

// Navigate
let mut params = RouteParams::new();
params.set("id".into(), "42".into());
params.set("post_id".into(), "7".into());
Navigator::push_named(cx, "user-post", &params);
// Navigates to: /users/42/posts/7

// Generate URL without navigating
let url = Navigator::url_for(cx, "user-post", &params);
// Some("/users/42/posts/7")
```

## Widgets

### RouterView

Top-level widget that renders the matched route at depth 0. Place this once at the root of your app:

```rust
impl Render for AppShell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(RouterView::new())
    }
}
```

Functional alternative:

```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div().size_full().child(router_view(window, cx))
}
```

### RouterOutlet

Renders child routes inside a parent layout. Each `RouterOutlet` increments the nesting depth:

```rust
// In a parent route's component
fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
        .child("Dashboard Header")
        .child(RouterOutlet::new())  // child routes render here
}
```

Named outlet for multiple content areas:

```rust
RouterOutlet::named("sidebar")
```

### RouterLink

Navigation link with automatic active-state detection:

```rust
fn navbar(cx: &mut Context<'_, MyView>) -> impl IntoElement {
    div().flex().gap_4()
        .child(
            RouterLink::new("/")
                .child(div().child("Home"))
                .active_class(|d| d.bg(rgb(0x2196f3)).text_color(white()))
                .build(cx)
        )
        .child(
            RouterLink::new("/settings")
                .child(div().child("Settings"))
                .active_class(|d| d.bg(rgb(0x2196f3)).text_color(white()))
                .build(cx)
        )
}
```

Shorthand:

```rust
router_link(cx, "/about", "About")
```

## Nested Routing

Define parent layouts with child routes that render inside `RouterOutlet`:

```rust
init_router(cx, |router| {
    router.add_route(
        Route::new("/dashboard", |_, _, _| dashboard_layout().into_any_element())
            .children(vec![
                Route::view("/dashboard/overview", || overview().into_any_element()).into(),
                Route::view("/dashboard/settings", || settings().into_any_element()).into(),
                Route::new("/dashboard/users/:id", |_, _, p| user_detail(p).into_any_element()).into(),
            ])
    );
});
```

Navigating to `/dashboard/settings` renders the dashboard layout with settings inside its `RouterOutlet`.

### Named Outlets

Route children into different content areas:

```rust
Route::new("/app", |_, _, _| app_shell().into_any_element())
    .children(vec![main_content.into()])
    .named_outlet("sidebar", vec![sidebar_nav.into()])
```

Render with `RouterOutlet::named("sidebar")` in the layout.

### Index Routes

A child route with path `""` (empty) acts as the index — it renders when the parent path is matched exactly:

```rust
Route::new("/dashboard", |_, _, _| layout().into_any_element())
    .children(vec![
        Route::view("", || index_page().into_any_element()).into(),       // /dashboard
        Route::view("settings", || settings().into_any_element()).into(), // /dashboard/settings
    ])
```

### Parameter Inheritance

Child routes automatically inherit parameters from all ancestor routes:

```rust
// Route: /orgs/:org_id/teams/:team_id/members/:member_id
// At the deepest child, params contains: org_id, team_id, member_id
```

If a child defines a parameter with the same name as a parent, the child value takes precedence (with a debug warning).

## Route Parameters

### Path Parameters

Define with `:name` syntax. Extract with `RouteParams`:

```rust
Route::new("/users/:id", |_, _, params| {
    let id = params.get("id").unwrap();

    // Type-safe extraction
    let id_num: Option<u64> = params.get_as("id");

    div().child(format!("User #{}", id)).into_any_element()
})
```

Construct programmatically:

```rust
let params = RouteParams::from_path("/users/42", "/users/:id");
assert_eq!(params.get("id"), Some(&"42".to_string()));
```

Merge parent and child params:

```rust
let merged = RouteParams::merge(&parent_params, &child_params);
```

### Query Parameters

Parse and serialize query strings:

```rust
let qp = QueryParams::from_query_string("?search=rust&page=2&tag=web&tag=api");

let search: Option<&String> = qp.get("search");
let page: Option<u32> = qp.get_as("page");
let tags: Option<&Vec<String>> = qp.get_all("tag"); // multi-value support

let qs: String = qp.to_query_string(); // "page=2&search=rust&tag=web&tag=api"
```

## Transitions

> Requires feature `transition` (enabled by default)

Add animations between route changes:

```rust
Route::view("/fade", || page().into_any_element())
    .transition(Transition::fade(300))

Route::view("/slide", || page().into_any_element())
    .transition(Transition::slide_left(400))
```

Available transitions:

| Constructor | Description |
|---|---|
| `Transition::None` | Instant switch, no animation |
| `Transition::fade(ms)` | Opacity cross-fade |
| `Transition::slide_left(ms)` | Slide from right to left |
| `Transition::slide_right(ms)` | Slide from left to right |
| `Transition::slide_up(ms)` | Slide from bottom to top |
| `Transition::slide_down(ms)` | Slide from top to bottom |

Override a transition for a single navigation:

```rust
Navigator::push_with_transition(cx, "/profile", Transition::fade(200));
Navigator::set_next_transition(cx, Transition::slide_up(300));
```

The library uses a **dual animation system**: the incoming route's transition drives both exit (old page) and enter (new page) animations simultaneously.

## Route Guards

> Requires feature `guard` (enabled by default)

Guards run before navigation and can allow, deny, or redirect:

```rust
use gpui_navigator::*;

// Authentication — redirect to /login if not authenticated
Route::new("/profile", handler)
    .guard(AuthGuard::new(|cx| is_logged_in(cx), "/login"))

// Role-based — require "admin" role
Route::new("/admin", handler)
    .guard(RoleGuard::new(|cx| get_role(cx), "admin", Some("/forbidden")))

// Permission-based
Route::new("/settings", handler)
    .guard(PermissionGuard::new(|cx, perm| check(cx, perm), "settings.edit")
        .with_redirect("/no-access"))

// Invert any guard
Route::new("/public-only", handler)
    .guard(NotGuard::new(AuthGuard::new(|cx| is_logged_in(cx), "/")))

// Custom guard with a closure
Route::new("/custom", handler)
    .guard(guard_fn(|cx, req| {
        if some_condition(cx) {
            NavigationAction::Continue
        } else {
            NavigationAction::redirect("/other")
        }
    }))
```

Guards have a `priority()` (higher runs first). Multiple guards on a route run in priority order; the first non-Continue result wins.

## Middleware

> Requires feature `middleware` (enabled by default)

Hooks that run before and after every navigation on the route:

```rust
use gpui_navigator::*;

// Using the trait
struct Analytics;

impl RouteMiddleware for Analytics {
    fn before_navigation(&self, _cx: &App, req: &NavigationRequest) {
        log::info!("navigating to {}", req.to);
    }
    fn after_navigation(&self, _cx: &App, req: &NavigationRequest) {
        log::info!("arrived at {}", req.to);
    }
    fn name(&self) -> &str { "Analytics" }
    fn priority(&self) -> i32 { 100 } // higher = runs earlier (before), later (after)
}

Route::view("/tracked", || page().into_any_element())
    .middleware(Analytics)

// Using closures
Route::view("/logged", || page().into_any_element())
    .middleware(middleware_fn(
        |_cx, req| log::info!("before: {}", req.to),
        |_cx, req| log::info!("after: {}", req.to),
    ))
```

## Route Lifecycle

Lifecycle hooks for fine-grained control over route activation/deactivation:

```rust
use gpui_navigator::*;

struct ConfirmExit;

impl RouteLifecycle for ConfirmExit {
    fn on_enter(&self, _cx: &App, _req: &NavigationRequest) -> NavigationAction {
        NavigationAction::Continue
    }
    fn on_exit(&self, _cx: &App) -> NavigationAction {
        NavigationAction::Continue
    }
    fn can_deactivate(&self, cx: &App) -> NavigationAction {
        if has_unsaved_changes(cx) {
            NavigationAction::deny("Unsaved changes")
        } else {
            NavigationAction::Continue
        }
    }
}

Route::view("/editor", || editor().into_any_element())
    .lifecycle(ConfirmExit)
```

`NavigationAction` variants:

| Variant | Effect |
|---|---|
| `NavigationAction::Continue` / `::allow()` | Allow navigation |
| `NavigationAction::deny(reason)` | Block navigation |
| `NavigationAction::redirect(path)` | Redirect to a different route |
| `NavigationAction::redirect_with_reason(path, reason)` | Redirect with explanation |

## Error Handling

Built-in styled 404 and error pages work out of the box. Customize them:

```rust
// Per-route error handlers
let handlers = ErrorHandlers::new()
    .on_not_found(|_cx, path| {
        div().child(format!("Nothing at {}", path)).into_any_element()
    })
    .on_error(|_cx, error| {
        div().child(format!("Error: {}", error)).into_any_element()
    });

// Global default pages
let pages = DefaultPages::new()
    .with_not_found(|| div().child("Custom 404").into_any_element())
    .with_error(|msg| div().child(format!("Error: {}", msg)).into_any_element())
    .with_loading(|| div().child("Loading...").into_any_element());
```

`NavigationResult` returned from navigation operations:

| Variant | Meaning |
|---|---|
| `Success { path }` | Route matched and rendered |
| `NotFound { path }` | No route matched the path |
| `Blocked { reason, redirect }` | Guard or lifecycle denied navigation |
| `Error(NavigationError)` | Internal error |

## Caching

> Requires feature `cache` (enabled by default, depends on `lru`)

Route resolution results are cached in an LRU cache:

```rust
// Access cache stats through the router
let stats: &CacheStats = router.cache_stats();

println!("Hit rate: {:.1}%", stats.overall_hit_rate() * 100.0);
println!("Parent hits: {}, misses: {}", stats.parent_hits, stats.parent_misses);
```

## Feature Flags

| Feature | Default | Description | Dependencies |
|---|---|---|---|
| `guard` | yes | `AuthGuard`, `RoleGuard`, `PermissionGuard`, `NotGuard`, `guard_fn` | — |
| `middleware` | yes | `RouteMiddleware` trait, `middleware_fn` helper | — |
| `transition` | yes | `Transition::fade`, `slide_left/right/up/down` | — |
| `cache` | yes | LRU route resolution cache | `lru` |
| `log` | yes | Logging via the `log` crate | `log` |
| `tracing` | no | Logging via `tracing` (mutually exclusive with `log`) | `tracing` |

## Examples

```bash
# Nested routing with parameter inheritance
cargo run --example nested_demo --all-features

# All transition types with live preview
cargo run --example transition_demo --all-features

# RouterLink, error pages, dynamic params
cargo run --example error_demo --all-features

# Stateful components with Entity caching
cargo run --example stateful_demo --all-features
```

## Architecture Overview

```
                    init_router()
                         |
                    GlobalRouter         (state.rs, context.rs)
                    /    |     \
              Routes   History  Cache    (route.rs, history.rs, cache.rs)
               |         |
          MatchStack  NavigationResult   (resolve.rs, error.rs)
               |
         RouterView / RouterOutlet       (widgets.rs)
               |
          Nested resolution              (nested.rs, params.rs)
```

**Core flow:**

1. `init_router` registers routes in a global `RouterState`
2. `Navigator::push("/path")` triggers route matching via `resolve_match_stack`
3. Guards and middleware run in priority order
4. `RouterView` renders the root match; `RouterOutlet` renders children at each nesting depth
5. Transitions animate between the old and new content

**Key modules:**

| Module | Responsibility |
|---|---|
| `context.rs` | `Navigator` static API, `init_router`, GPUI integration |
| `route.rs` | `Route` builder, `RouteConfig`, named route registry |
| `resolve.rs` | `MatchStack` resolution — maps a path to a chain of matched routes |
| `nested.rs` | Child route resolution, path normalization, parameter extraction |
| `widgets.rs` | `RouterView`, `RouterOutlet`, `RouterLink`, `DefaultPages` |
| `params.rs` | `RouteParams` (path), `QueryParams` (query string) |
| `state.rs` | `RouterState` — centralized navigation state |
| `history.rs` | Navigation history stack with back/forward |
| `guards.rs` | `RouteGuard` trait and built-in implementations |
| `middleware.rs` | `RouteMiddleware` trait with priority ordering |
| `transition.rs` | `Transition` enum and `TransitionConfig` |
| `lifecycle.rs` | `RouteLifecycle` trait, `NavigationAction` enum |
| `cache.rs` | LRU cache for route resolution with `CacheStats` |
| `error.rs` | `NavigationError`, `NavigationResult`, `ErrorHandlers` |
| `logging.rs` | Unified logging macros (`log` / `tracing` backends) |

## API Reference

Full API documentation is available on [docs.rs](https://docs.rs/gpui-navigator).

### Quick Reference

| Type / Function | Description |
|---|---|
| `init_router(cx, \|router\| { ... })` | Register routes globally |
| `Navigator::push(cx, path)` | Navigate to a path |
| `Navigator::replace(cx, path)` | Replace current path |
| `Navigator::pop(cx)` | Go back |
| `Navigator::forward(cx)` | Go forward |
| `Navigator::current_path(cx)` | Get current path |
| `Navigator::push_named(cx, name, params)` | Navigate by route name |
| `Navigator::of(cx).push(p).push(p2)` | Fluent chaining |
| `Route::view(path, closure)` | Stateless route |
| `Route::component(path, factory)` | Stateful route (Entity cached) |
| `Route::component_with_params(path, factory)` | Stateful + params |
| `Route::new(path, handler)` | Full-control route |
| `.children(vec![...])` | Add child routes |
| `.name("n")` | Name the route |
| `.transition(Transition::fade(ms))` | Add transition |
| `.guard(AuthGuard::new(check, redirect))` | Add guard |
| `.middleware(impl RouteMiddleware)` | Add middleware |
| `.lifecycle(impl RouteLifecycle)` | Add lifecycle hooks |
| `RouterView::new()` | Root route renderer |
| `RouterOutlet::new()` | Child route renderer |
| `RouterOutlet::named("n")` | Named outlet |
| `RouterLink::new(path).child(el).build(cx)` | Nav link |
| `RouteParams::get("key")` | Get path param |
| `RouteParams::get_as::<T>("key")` | Typed extraction |
| `QueryParams::from_query_string(qs)` | Parse query string |
| `DefaultPages::new().with_not_found(f)` | Custom error pages |

## Minimum Supported Rust Version

Rust 1.75 or later.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new features
4. Run `cargo test --all-features && cargo clippy --all-targets --all-features && cargo fmt --check`
5. Open a Pull Request
