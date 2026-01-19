# GPUI Navigator

[![Crates.io](https://img.shields.io/crates/v/gpui-navigator.svg)](https://crates.io/crates/gpui-navigator)
[![Documentation](https://docs.rs/gpui-navigator/badge.svg)](https://docs.rs/gpui-navigator)
[![License](https://img.shields.io/crates/l/gpui-navigator.svg)](LICENSE-MIT)

A declarative navigation library for [GPUI](https://gpui.rs) with smooth transitions, nested routing, and beautiful default error pages.

## Features

- 🎨 **Smooth Transitions** - Fade, slide animations with dual enter/exit support
- 🔀 **Nested Routing** - Parent/child route hierarchies with `RouterOutlet`
- 🎯 **Simple API** - Intuitive route definition with closures
- 🖼️ **Beautiful Defaults** - Pre-styled 404 and error pages out of the box
- 🔗 **RouterLink Widget** - Navigation links with active state styling
- 🛡️ **Route Guards** - Authentication and authorization (optional)
- 🔌 **Middleware** - Before/after navigation hooks (optional)
- 📝 **Named Routes** - Navigate by name instead of path

## Why GPUI Navigator?

Unlike other GPUI routers:

- **Zero Boilerplate** - Define routes with simple closures, no complex builders
- **Smooth Animations** - Dual-animation system for professional transitions
- **Production Ready** - Beautiful error pages included, not placeholder text
- **Developer Experience** - Clean API inspired by modern web frameworks

## Installation

```toml
[dependencies]
gpui-navigator = "0.1"
gpui = "0.2"
```

## Quick Start

```rust
use gpui::prelude::*;
use gpui::*;
use gpui_navigator::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        // Initialize router
        init_router(cx, |router| {
            // Define routes with closures
            router.add_route(
                Route::new("/", |_, _| home_page().into_any_element())
                    .transition(Transition::fade(300))
            );
            
            router.add_route(
                Route::new("/about", |_, _| about_page().into_any_element())
                    .transition(Transition::slide_left(400))
            );
        });

        // Open window with RouterOutlet
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|cx| AppView::new(cx))
        }).unwrap();
    });
}

fn home_page() -> impl IntoElement {
    div().child("Home Page")
}

fn about_page() -> impl IntoElement {
    div().child("About Page")
}

struct AppView {
    outlet: Entity<RouterOutlet>,
}

impl AppView {
    fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            outlet: cx.new(|_| RouterOutlet::new()),
        }
    }
}

impl Render for AppView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.outlet.clone())
    }
}
```

## Navigation

### Programmatic Navigation

```rust
use gpui_navigator::Navigator;

// Push new route
Navigator::push(cx, "/about");

// Replace current route
Navigator::replace(cx, "/login");

// Go back
Navigator::pop(cx);

// Go forward  
Navigator::forward(cx);

// Get current path
let path = Navigator::current_path(cx);

// Check if can go back
if Navigator::can_pop(cx) {
    Navigator::pop(cx);
}
```

### RouterLink Widget

Create clickable navigation links with automatic active state:

```rust
use gpui_navigator::*;

fn navbar(cx: &mut Context<'_, AppView>) -> impl IntoElement {
    div()
        .flex()
        .gap_4()
        // Basic link
        .child(
            RouterLink::new("/".to_string())
                .child(div().child("Home"))
                .build(cx)
        )
        // Link with active styling
        .child(
            RouterLink::new("/about".to_string())
                .child(div().px_4().py_2().child("About"))
                .active_class(|div| {
                    div.bg(rgb(0x2196f3))
                       .text_color(white())
                })
                .build(cx)
        )
}
```

**RouterLink features:**
- ✅ Instant navigation with immediate UI updates
- ✅ Automatic active state detection
- ✅ Customizable active styling
- ✅ Works with nested routes

## Route Transitions

Add smooth animations between pages:

```rust
use gpui_navigator::*;

// Fade transition
Route::new("/fade", |_, _| page().into_any_element())
    .transition(Transition::fade(300))

// Slide transitions
Route::new("/slide-left", |_, _| page().into_any_element())
    .transition(Transition::slide_left(400))

Route::new("/slide-right", |_, _| page().into_any_element())
    .transition(Transition::slide_right(400))

Route::new("/slide-up", |_, _| page().into_any_element())
    .transition(Transition::slide_up(400))

Route::new("/slide-down", |_, _| page().into_any_element())
    .transition(Transition::slide_down(400))

// No transition
Route::new("/instant", |_, _| page().into_any_element())
    .transition(Transition::None)
```

**Dual Animation System:**
GPUI Navigator uses the new route's transition for both exit and enter animations, creating smooth, professional transitions.

## Route Parameters

Extract dynamic values from URLs:

```rust
use gpui_navigator::*;

// Define route with parameter
router.add_route(
    Route::new("/users/:id", |_, params| {
        user_page(params).into_any_element()
    })
);

fn user_page(params: &RouteParams) -> impl IntoElement {
    let user_id = params.get("id").unwrap_or(&"unknown".to_string());
    div().child(format!("User: {}", user_id))
}

// Navigate with parameter
Navigator::push(cx, "/users/123");
```

## Nested Routes

Create layouts with child routes:

```rust
use gpui_navigator::*;

router.add_route(
    Route::new("/dashboard", |_, _| dashboard_layout().into_any_element())
        .children(vec![
            Route::new("overview", |_, _| overview_page().into_any_element()).into(),
            Route::new("settings", |_, _| settings_page().into_any_element()).into(),
        ])
);

fn dashboard_layout() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .child("Dashboard Header")
        .child(RouterOutlet::new())  // Child routes render here
}
```

Access nested routes:
- `/dashboard` - Shows dashboard layout
- `/dashboard/overview` - Shows overview inside layout
- `/dashboard/settings` - Shows settings inside layout

## Error Handling

### Default Error Pages

GPUI Navigator includes beautiful, pre-styled error pages:

- **404 Page** - Shown when no route matches (styled with red badge)
- **Loading Page** - Optional loading state
- **Error Page** - Generic error display

These work automatically - no configuration needed!

### Custom Error Pages

Override defaults if desired:

```rust
use gpui_navigator::*;

let default_pages = DefaultPages::new()
    .with_not_found(|| {
        div()
            .child("Custom 404")
            .child("Page not found")
            .into_any_element()
    });
```

## Named Routes

Navigate by name instead of hardcoded paths:

```rust
// Define named route
router.add_route(
    Route::new("/users/:id", |_, params| user_page(params).into_any_element())
        .name("user-profile")
);

// Navigate by name
let mut params = RouteParams::new();
params.set("id".to_string(), "123".to_string());
Navigator::push_named(cx, "user-profile", &params);
```

## Optional Features

Enable advanced features in `Cargo.toml`:

```toml
[dependencies]
gpui-navigator = { version = "0.1", features = ["guard", "middleware", "cache"] }
```

### Route Guards

Protect routes with authentication:

```rust
#[cfg(feature = "guard")]
use gpui_navigator::*;

fn is_logged_in(cx: &App) -> bool {
    // Check auth state
    true
}

Route::new("/profile", |_, _| profile_page().into_any_element())
    .guard(AuthGuard::new(is_logged_in, "/login"))
```

### Middleware

Add hooks before/after navigation:

```rust
#[cfg(feature = "middleware")]
use gpui_navigator::*;

struct LoggingMiddleware;

impl RouteMiddleware for LoggingMiddleware {
    // Implement before_navigation and after_navigation
}

Route::new("/", |_, _| home().into_any_element())
    .middleware(LoggingMiddleware)
```

## Examples

Run the included examples:

```bash
# Transition animations demo
cargo run --example transition_demo

# RouterLink and error handling demo
cargo run --example error_demo
```

## API Summary

| Function/Type | Description |
|--------------|-------------|
| `init_router(cx, \|router\| {...})` | Initialize the router with routes |
| `Route::new(path, handler)` | Create a new route |
| `.transition(Transition::fade(ms))` | Add transition animation |
| `.name("route-name")` | Name the route for reference |
| `.children(vec![...])` | Add child routes |
| `Navigator::push(cx, path)` | Navigate to path |
| `Navigator::pop(cx)` | Go back |
| `RouterOutlet::new()` | Render current/child routes |
| `RouterLink::new(path)` | Create navigation link |
| `RouteParams::get("key")` | Get route parameter |

## Architecture

GPUI Navigator is built with a clean, modular architecture:

- **Core**: Route matching, state management, navigation
- **Widgets**: RouterOutlet (route renderer), RouterLink (nav links)
- **Optional**: Guards, middleware, caching (feature-gated)
- **Defaults**: Beautiful error pages included

## Minimum Supported Rust Version

Rust 1.75 or later.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions welcome! Please:

1. Fork the repository
2. Create your feature branch
3. Add tests for new features
4. Ensure all tests pass: `cargo test`
5. Submit a Pull Request

## Acknowledgments

- Built for [GPUI](https://gpui.rs) by Zed Industries
- Inspired by modern web routing libraries
