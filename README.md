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
  - [Route::component_with_context — Pages That Need Window/Context](#routecomponent_with_context--pages-that-need-windowcontext)
  - [Route::new — Full Control](#routenew--full-control)
- [Navigation API](#navigation-api)
  - [Programmatic Navigation](#programmatic-navigation)
  - [Fluent API](#fluent-api)
  - [Named Routes](#named-routes)
  - [Typed Routes](#typed-routes)
- [Widgets](#widgets)
  - [RouterView](#routerview)
  - [RouterOutlet](#routeroutlet)
  - [RouterLink](#routerlink)
- [Nested Routing](#nested-routing)
  - [Named Outlets](#named-outlets)
  - [Index Routes](#index-routes)
  - [Parameter Inheritance](#parameter-inheritance)
- [Route Parameters](#route-parameters)
  - [Path Patterns](#path-patterns)
  - [Path Parameters](#path-parameters)
  - [Query Parameters](#query-parameters)
- [Transitions](#transitions)
- [Route Guards](#route-guards)
  - [Async Guards](#async-guards)
- [Middleware](#middleware)
- [Route Lifecycle](#route-lifecycle)
  - [Asking the Page: `can_leave`](#asking-the-page-can_leave)
- [Desktop Navigation](#desktop-navigation)
- [Error Handling](#error-handling)
- [Caching](#caching)
- [Feature Flags](#feature-flags)
- [Examples](#examples)
- [Architecture Overview](#architecture-overview)
- [API Reference](#api-reference)
- [License](#license)

## Features

- **Smooth Transitions** — Fade, slide (4 directions), easing curves and custom transitions; slides mirror when going back
- **Nested Routing** — Unlimited nesting depth with `RouterOutlet`, named outlets, index routes
- **Stateful Components** — `Route::component()` auto-caches GPUI entities across navigations
- **Route Guards** — `AuthGuard`, `RoleGuard`, `PermissionGuard`, composable `NotGuard`, async guards
- **Path Patterns** — `:params`, typed constraints (`:id<int>`), `*` catch-alls; query strings just work
- **Middleware** — Before/after navigation hooks with priority ordering
- **Named Routes** — Navigate by name with parameter substitution
- **Route Lifecycle** — `on_enter`, `on_exit`, `can_deactivate` hooks on every nesting level, plus `can_leave` that asks the page itself
- **Desktop Navigation** — back/forward key bindings, mouse side buttons, route change events, history persistence
- **LRU Cache** — Route resolution caching with hit-rate stats
- **Error Pages** — Built-in styled 404 and error pages, fully customizable
- **RouterLink** — Navigation links with automatic active-state styling, exact or per-section
- **Type-safe Params** — `get_as::<T>()` for parsed parameter extraction
- **Logging** — `log` or `tracing` backend (mutually exclusive, feature-gated)

## Installation

```toml
[dependencies]
gpui-navigator = "0.2.1"
gpui = "0.2"
```

All features are enabled by default. To pick only what you need, disable the defaults and
list a GPUI backend plus the modules you want:

```toml
[dependencies]
gpui-navigator = { version = "0.2", default-features = false, features = ["gpui-02", "transition", "guard"] }
```

### GPUI backends

gpui-navigator works with three flavours of GPUI. Enable exactly one backend feature, the
one matching the `gpui` your app (and [gpui-component](https://github.com/longbridge/gpui-component),
if you use it) depends on:

| Feature | Default | Backend crate | Pairs with |
|---|---|---|---|
| `gpui-02` | yes | [`gpui`](https://crates.io/crates/gpui) 0.2 (Zed's published release) | gpui-component 0.5.x |
| `gpui-pre` | no | [`gpui-pre`](https://crates.io/crates/gpui-pre) (snapshots of current gpui from the Zed repo) | gpui-component 0.6.x |
| `gpui-fast` | no | [`gpui-fast`](https://crates.io/crates/gpui-fast) (fork used by newer gpui-component) | gpui-component 0.7.x |

```toml
[dependencies]
gpui-navigator = { version = "0.2", default-features = false, features = ["gpui-pre", "log", "guard", "middleware", "cache", "transition"] }
gpui = { package = "gpui-pre", version = "0.3" }
```

The backends are mutually exclusive: selecting `gpui-pre` or `gpui-fast` requires
`default-features = false`, otherwise the build fails with an explanatory error.

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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(router_view(window, cx))
    }
}
```

`init_router` registers routes globally. `router_view` at the top level renders whichever route matches the current path.

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

### `Route::component_with_context` — Pages That Need Window/Context

Use this when the page creates child entities in its constructor, such as
[gpui-component](https://github.com/longbridge/gpui-component)'s `InputState`. The factory
receives `&mut Window` and the new entity's `&mut Context<Self>`, and the entity is cached
like with `component`, so typed text survives re-renders and navigation:

```rust
use gpui_component::input::{Input, InputState};

struct HomePage { input: Entity<InputState> }

impl HomePage {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Enter your name..."));
        Self { input }
    }
}

impl Render for HomePage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(Input::new(&self.input))
    }
}

Route::component_with_context("/", HomePage::new)
```

`Route::component_with_params_and_context(path, |params, window, cx| ...)` is the
variant that also receives route parameters.

Don't call `cx.new(...)` inside a `Route::new` builder: the builder runs on every render, so
the entity (and anything typed into it) would be recreated each frame.

> **Using gpui-component?** Its widgets expect the window's root view to be
> `gpui_component::Root`. Wrap your app view when opening the window, otherwise
> `Input` and friends panic with ``called `Option::unwrap()` on a `None` value`` in `root.rs`:
>
> ```rust
> cx.open_window(WindowOptions::default(), |window, cx| {
>     let view = cx.new(|_| AppShell);
>     cx.new(|cx| gpui_component::Root::new(view, window, cx))
> })
> ```

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
let path: String = Navigator::current_path(cx);   // includes any "?query"
let page: Option<u32> = Navigator::query(cx).get_as("page");
let id: Option<u64> = Navigator::params(cx).get_as("id");
let can_back: bool = Navigator::can_pop(cx);
let can_fwd: bool = Navigator::can_go_forward(cx);
```

Every method has a `try_` variant (`try_push`, `try_replace`, `try_back`, `try_forward`,
`try_push_named`) that returns the [`NavigationResult`](#error-handling), so you can tell
when a guard blocked the navigation:

```rust
match Navigator::try_push(cx, "/admin") {
    NavigationResult::Success { .. } => {}
    NavigationResult::Blocked { reason, .. } => show_toast(cx, reason),
    NavigationResult::Pending { .. } => {} // async guards are running
    _ => {}
}
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

Names work on nested routes too: `url_for` builds the full path from the parent routes.
It returns `None` if the name is unknown or a parameter is missing.

### Typed Routes

Describe your screens as an enum and implement `IntoRoute`; every `Navigator` method
accepts it, so a typo in a path becomes a compile error:

```rust
enum AppRoute {
    Home,
    Project { id: u32 },
    Projects { page: u32 },
}

impl IntoRoute for AppRoute {
    fn into_route(self) -> RouteDescriptor {
        match self {
            Self::Home => "/".into_route(),
            Self::Project { id } => format!("/projects/{id}").into_route(),
            Self::Projects { page } => format!("/projects?page={page}").into_route(),
        }
    }
}

Navigator::push(cx, AppRoute::Project { id: 7 });
```

## Widgets

### RouterView

Top-level widget that renders the matched route at depth 0. Place this once at the root of your app:

```rust
impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(router_view(window, cx))
    }
}
```

`RouterView` is also available as a view: keep an `Entity<RouterView>` (created once with
`cx.new(|_| RouterView::new())`) and pass it to `.child(...)`.

### RouterOutlet

Renders child routes inside a parent layout. Each `RouterOutlet` increments the nesting depth:

```rust
// In a parent route's component
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .child("Dashboard Header")
        .child(router_outlet(window, cx, "dashboard-outlet"))  // child routes render here
}
```

Named outlet for multiple content areas:

```rust
router_outlet_named(window, cx, "dashboard-sidebar", "sidebar")
```

Or hold the outlet as an entity: `cx.new(|_| RouterOutlet::new())` / `RouterOutlet::named("sidebar")`.

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

Links navigate when the mouse button is released over them. By default a link is active only
on its exact path (the query string is ignored); use `ActiveMatch::Prefix` for a sidebar entry
that should stay highlighted for a whole section:

```rust
RouterLink::new("/settings")
    .active_match(ActiveMatch::Prefix) // also active on /settings/profile
    .child("Settings")
    .build(cx)
```

`is_link_active(current, target, mode)` exposes the same check for your own widgets (for
example gpui-component `Button`s, see `examples/gpui-component`).

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
                Route::view("overview", || overview().into_any_element()).into(),
                Route::view("settings", || settings().into_any_element()).into(),
                Route::new("users/:id", |_, _, p| user_detail(p).into_any_element()).into(),
            ])
    );
});
```

Child paths are relative to their parent. Navigating to `/dashboard/settings` renders the
dashboard layout with settings inside its `RouterOutlet`.

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

### Path Patterns

| Segment | Matches | Captured as |
|---|---|---|
| `users` | exactly `users` | — |
| `:id` | any one segment | `id` |
| `:id<int>` | one segment that satisfies the constraint | `id` |
| `*rest` | the rest of the path (zero or more segments) | `rest` |
| `*` | the rest of the path | `*` |

Constraints: `int`/`i64`, `i32`, `uint`/`u64`, `u32`, `usize`, `float`/`f64`, `bool`, `uuid`,
`alpha`, `alnum`, `slug`. A segment that fails its constraint lets the next route try, so
`/posts/:id<int>` and `/posts/:slug` can live side by side. A wildcard must be the last
segment; `Route::view("*", ...)` registered last makes a catch-all 404 page.

Routes are tried in registration order. When a route can never match because an earlier
sibling already covers all its paths (`/:id` before `/about`), a warning is logged at
registration.

The query string (`?…`) and fragment (`#…`) never take part in matching:
`/users?page=2` renders the `/users` route, and `Navigator::query(cx)` returns `page=2`.

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

Read the current location's query with `Navigator::query(cx)` (or
`QueryParams::from_location(path)`). Parse and serialize query strings directly:

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

Ease any transition, or style the entering and leaving page yourself (progress goes 0 → 1):

```rust
Transition::slide_left(250).with_easing(ease_out_cubic)

Transition::custom(
    300,
    |page, p| page.opacity(p).top(px(24.0 * (1.0 - p))),
    |page, p| page.opacity(1.0 - p),
)
```

Transitions are plain data (functions, not capturing closures), so they fit in `const` items.
When navigating back, slides play mirrored (a page that slid in from the right leaves to the
right); turn this off with `router.set_reverse_transitions_on_back(false)`.

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

Guards have a `priority()` (higher runs first). Guards are collected from every level of the
route being opened (a guard on `/dashboard` also protects `/dashboard/settings`) and run in
priority order; the first non-Continue result wins. A redirect from a `replace` stays a replace,
so it does not add a history entry.

### Async Guards

When the decision needs I/O (refreshing a session token, asking a server), use
`async_guard_fn`. The navigation reports `NavigationResult::Pending` and the current page stays
until the future resolves; a newer navigation cancels it.

```rust
Route::view("/account", account_page)
    .guard(async_guard_fn(|cx, _request| {
        let session = cx.global::<Session>().clone();
        async move {
            if session.refresh().await.is_ok() {
                NavigationAction::Continue
            } else {
                NavigationAction::redirect("/login")
            }
        }
    }))
```

The future runs on GPUI's foreground executor (move heavy work to
`cx.background_executor()`). `Navigator::is_pending(cx)` tells you whether to show a spinner.
Async guards run when navigating through `Navigator` or `navigate`; custom guards can
implement `RouteGuard::check_async` directly.

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

Hooks run on every nesting level that changes: navigating from `/settings/profile` to
`/settings/account` calls `on_exit` on `profile` and `on_enter` on `account`, while `settings`
stays mounted. The full order of a navigation is:

1. guards of the target route and its parents (sync, then async)
2. `can_deactivate` on the levels being left (deepest first)
3. `on_exit` on the levels being left (deepest first)
4. `before_navigation` middleware
5. history update
6. `on_enter` on the levels being entered (outermost first); a deny undoes the navigation
7. `after_navigation` middleware

### Asking the Page: `can_leave`

For `Route::component*` pages, `can_leave` hands you the page entity itself, so the check can
look at the page's own state:

```rust
Route::component_with_context("/editor", Editor::new)
    .can_leave(|editor: &Editor, _cx| {
        if editor.is_dirty() {
            NavigationAction::deny("Unsaved changes")
        } else {
            NavigationAction::Continue
        }
    })
```

`NavigationAction` variants:

| Variant | Effect |
|---|---|
| `NavigationAction::Continue` / `::allow()` | Allow navigation |
| `NavigationAction::deny(reason)` | Block navigation |
| `NavigationAction::redirect(path)` | Redirect to a different route |
| `NavigationAction::redirect_with_reason(path, reason)` | Redirect with explanation |

## Desktop Navigation

```rust
Application::new().run(|cx: &mut App| {
    init_router(cx, |router| { /* routes */ });

    // alt-left / alt-right everywhere, plus cmd-[ / cmd-] on macOS
    bind_navigation_keys(cx);

    // Fires once per completed navigation (not on re-renders)
    on_route_change(cx, |event, cx| {
        log::info!("{:?} {} → {}", event.direction, event.from.as_deref().unwrap_or("-"), event.to);
    })
    .detach();
});

// Mouse back/forward side buttons over the root view
impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        with_mouse_navigation(div().size_full())
            .child(router_view(window, cx))
            .child(router_debug_panel(cx)) // location, match stack, history, cache
    }
}
```

`GoBack` / `GoForward` are regular GPUI actions: bind your own keys with
`register_navigation_actions(cx)` and `cx.bind_keys(...)`.

Save and restore history between launches (`HistorySnapshot` implements serde's traits with
the `serde` feature):

```rust
let snapshot = cx.global::<GlobalRouter>().history_snapshot();
// ... later
cx.update_global::<GlobalRouter, _>(|router, _| router.restore_history(snapshot));
```

`HistoryState` can carry small typed values with each entry:

```rust
Navigator::push_with_state(cx, "/doc/7", HistoryState::new().with("scroll_y", 420));
let scroll: Option<f32> = Navigator::current_entry(cx).state.and_then(|s| s.get_as("scroll_y"));
```

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
| `Pending { path }` | Waiting on async guards; finishes on its own |
| `Error(NavigationError)` | Internal error |

## Caching

### Page Entities

`Route::component*` pages are cached so their state survives navigation. The cache keeps the
128 most recently shown pages by default; tune it per route or globally:

```rust
// Start fresh every time the user opens the page
Route::component("/search", SearchPage::new).keep_alive(false)

router.set_component_cache_capacity(32);
router.evict_components("/users/:id"); // drop all cached user pages
router.clear_component_cache();
```

### Route Resolution

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
| `serde` | no | `Serialize`/`Deserialize` for `HistoryEntry`, `HistoryState`, `HistorySnapshot` | `serde` |

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

# AuthGuard, RoleGuard, PermissionGuard, guard_fn
cargo run --example guard_demo --all-features

# RouteMiddleware trait, middleware_fn, execution logging
cargo run --example middleware_demo --all-features

# RouteLifecycle: on_enter, on_exit, can_deactivate
cargo run --example lifecycle_demo --all-features

# Typed routes, keyboard/mouse back-forward, async guard, can_leave, debug panel
cargo run --example desktop_demo
```

`examples/gpui-component` is a standalone app (its own `Cargo.toml`) that combines the router
with [gpui-component](https://github.com/longbridge/gpui-component) widgets: a sidebar of
buttons, nested settings routes and a form whose `Input` keeps its text across navigation:

```bash
cd examples/gpui-component && cargo run
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
3. Guards, lifecycle hooks and middleware of the matched levels run (see [Route Lifecycle](#route-lifecycle))
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
| `matcher.rs` | Path pattern syntax: params, constraints, wildcards, query splitting |
| `actions.rs` | `GoBack`/`GoForward` actions, key bindings, mouse buttons, `on_route_change` |
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
| `Navigator::try_push(cx, path)` | Navigate and get the `NavigationResult` |
| `Navigator::query(cx)` / `Navigator::params(cx)` | Current query / route params |
| `Navigator::of(cx).push(p).push(p2)` | Fluent chaining |
| `Route::view(path, closure)` | Stateless route |
| `Route::component(path, factory)` | Stateful route (Entity cached) |
| `Route::component_with_params(path, factory)` | Stateful + params |
| `Route::component_with_context(path, \|window, cx\| ...)` | Stateful, built with Window/Context |
| `Route::component_with_params_and_context(path, \|params, window, cx\| ...)` | Stateful + params, built with Window/Context |
| `Route::new(path, handler)` | Full-control route |
| `.children(vec![...])` | Add child routes |
| `.name("n")` | Name the route |
| `.transition(Transition::fade(ms))` | Add transition |
| `.guard(AuthGuard::new(check, redirect))` | Add guard |
| `.middleware(impl RouteMiddleware)` | Add middleware |
| `.lifecycle(impl RouteLifecycle)` | Add lifecycle hooks |
| `.can_leave(\|page: &T, cx\| ...)` | Ask the page before leaving it |
| `.keep_alive(false)` | Drop the page entity when leaving |
| `.guard(async_guard_fn(...))` | Async guard |
| `bind_navigation_keys(cx)` | Back/forward shortcuts |
| `on_route_change(cx, f)` | Callback after each navigation |
| `router_view(window, cx)` | Root route renderer (`RouterView` as a view) |
| `router_outlet(window, cx, key)` | Child route renderer (`RouterOutlet` as a view) |
| `RouterOutlet::named("n")` | Named outlet |
| `RouterLink::new(path).child(el).build(cx)` | Nav link |
| `RouteParams::get("key")` | Get path param |
| `RouteParams::get_as::<T>("key")` | Typed extraction |
| `QueryParams::from_query_string(qs)` | Parse query string |
| `DefaultPages::new().with_not_found(f)` | Custom error pages |

## Minimum Supported Rust Version

Rust 1.88 or later.

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
