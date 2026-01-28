# API Contract: Nested Routing

**Feature**: 001-nested-routing-redesign  
**Version**: 1.0.0  
**Date**: 2026-01-28

## Overview

This document defines the public API surface for the redesigned nested routing system. All APIs follow the Constitution Principle I (API-First Design) - clean, intuitive, minimal boilerplate.

---

## Core API: Route Definition

### Route::new()

**Purpose**: Create a route with function-based builder

**Signature**:
```rust
pub fn new<F>(
    path: impl Into<String>,
    builder: F
) -> Self
where
    F: Fn(&mut Window, &mut App, &RouteParams) -> AnyElement + Send + Sync + 'static
```

**Usage**:
```rust
Route::new("/dashboard", |window, cx, params| {
    div().child("Dashboard").into_any_element()
})
```

**Guarantees**:
- Builder called only when route is active
- Params merged from parent routes
- Thread-safe (Send + Sync)

---

### Route::component()

**Purpose**: Create route with stateful component (zero parameters)

**Signature**:
```rust
pub fn component<T, F>(
    path: impl Into<String>, 
    create: F
) -> Self
where
    T: Render + 'static,
    F: Fn() -> T + Send + Sync + 'static + Clone
```

**Usage**:
```rust
Route::component("/counter", CounterPage::new)
```

**Guarantees**:
- Component instance cached across navigations (via GPUI use_keyed_state)
- State preserved when navigating away and back (within LRU cache limit)
- Constructor called only once per route instance

---

### Route::component_with_params()

**Purpose**: Create route with stateful component that receives params

**Signature**:
```rust
pub fn component_with_params<T, F>(
    path: impl Into<String>, 
    create: F
) -> Self
where
    T: Render + 'static,
    F: Fn(&RouteParams) -> T + Send + Sync + 'static + Clone
```

**Usage**:
```rust
Route::component_with_params("/user/:id", |params| {
    let id = params.get("id").unwrap();
    UserPage::new(id.clone())
})
```

**Guarantees**:
- Different param values create separate component instances
- Each instance cached independently
- Params include inherited parent params

---

### Route::children()

**Purpose**: Add child routes to create nested hierarchy

**Signature**:
```rust
pub fn children(mut self, children: Vec<Route>) -> Self
```

**Usage**:
```rust
Route::new("/dashboard", dashboard_layout)
    .children(vec![
        Route::component("overview", OverviewPage::new),
        Route::component("settings", SettingsPage::new),
        Route::new("", index_page), // Index route
    ])
```

**Guarantees**:
- Children paths are relative to parent
- Empty path "" creates index route (default child)
- Circular dependencies detected at construction time

---

### Route::name()

**Purpose**: Assign name for programmatic navigation

**Signature**:
```rust
pub fn name(mut self, name: impl Into<String>) -> Self
```

**Usage**:
```rust
Route::component("/dashboard", DashboardPage::new)
    .name("dashboard")

// Later:
Navigator::push_named(cx, "dashboard", RouteParams::new());
```

**Guarantees**:
- Names must be unique within route tree
- Enables refactoring paths without breaking navigation calls

---

### Route::transition() (Optional Feature)

**Purpose**: Configure route transition animation

**Signature**:
```rust
#[cfg(feature = "transition")]
pub fn transition(mut self, config: TransitionConfig) -> Self
```

**Usage**:
```rust
Route::component("/analytics", AnalyticsPage::new)
    .transition(TransitionConfig::slide(SlideDirection::Left, 300))
```

**Guarantees**:
- Only applies when navigating TO this route
- Exit transition uses parent's or default configuration

---

## Navigator API

### Navigator::push()

**Purpose**: Navigate to new path (adds history entry)

**Signature**:
```rust
pub fn push(cx: &mut App, path: impl Into<String>)
```

**Usage**:
```rust
Navigator::push(cx, "/dashboard/analytics");
```

**Behavior**:
- Adds new entry to history stack
- Triggers route resolution → component render
- Cancels any in-flight navigation
- Calls `cx.notify()` to trigger re-render

---

### Navigator::replace()

**Purpose**: Navigate without adding history entry

**Signature**:
```rust
pub fn replace(cx: &mut App, path: impl Into<String>)
```

**Usage**:
```rust
Navigator::replace(cx, "/login"); // Redirect without back button
```

**Behavior**:
- Replaces current history entry
- Useful for redirects and login flows

---

### Navigator::push_named()

**Purpose**: Navigate to named route with params

**Signature**:
```rust
pub fn push_named(
    cx: &mut App, 
    name: impl AsRef<str>,
    params: RouteParams
)
```

**Usage**:
```rust
let mut params = RouteParams::new();
params.set("id", "123");
Navigator::push_named(cx, "user-detail", params);
```

**Behavior**:
- Resolves name to path pattern
- Substitutes params into path
- Navigates to resolved path

---

### Navigator::back()

**Purpose**: Navigate to previous history entry

**Signature**:
```rust
pub fn back(cx: &mut App) -> bool
```

**Returns**: `true` if navigation occurred, `false` if at history start

**Usage**:
```rust
if Navigator::back(cx) {
    println!("Navigated back");
} else {
    println!("Already at start");
}
```

---

### Navigator::forward()

**Purpose**: Navigate to next history entry

**Signature**:
```rust
pub fn forward(cx: &mut App) -> bool
```

**Returns**: `true` if navigation occurred, `false` if at history end

---

### Navigator::clear_cache() (Optional Feature)

**Purpose**: Explicitly remove route from component cache

**Signature**:
```rust
#[cfg(feature = "cache")]
pub fn clear_cache(cx: &mut App, path: &str)
```

**Usage**:
```rust
// Force re-creation on next visit
Navigator::clear_cache(cx, "/dashboard/analytics");
```

**Behavior**:
- Removes cached Entity for path
- Next navigation to path will reconstruct component
- Useful for forced resets or logout flows

---

### Navigator::clear_all_cache() (Optional Feature)

**Purpose**: Clear entire component cache

**Signature**:
```rust
#[cfg(feature = "cache")]
pub fn clear_all_cache(cx: &mut App)
```

**Usage**:
```rust
// On logout
Navigator::clear_all_cache(cx);
Navigator::replace(cx, "/login");
```

---

## RouterOutlet Component

### RouterOutlet::new()

**Purpose**: Create default (unnamed) outlet

**Signature**:
```rust
pub fn new() -> Self
```

**Usage**:
```rust
impl Render for DashboardLayout {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child("Dashboard Header")
            .child(cx.new(|_| RouterOutlet::new())) // Child routes render here
    }
}
```

**Behavior**:
- Renders first matched child route
- If no child matches, renders nothing
- Automatically manages child component lifecycle

---

### RouterOutlet::named()

**Purpose**: Create named outlet for parallel child routes

**Signature**:
```rust
pub fn named(name: impl Into<String>) -> Self
```

**Usage**:
```rust
div()
    .child(RouterOutlet::new())              // Main outlet
    .child(RouterOutlet::named("sidebar"))   // Sidebar outlet
```

**Behavior**:
- Routes can specify target outlet via configuration
- Enables multiple child routes visible simultaneously
- Each outlet manages own child lifecycle independently

---

## RouteParams API

### RouteParams::new()

**Purpose**: Create empty params map

**Signature**:
```rust
pub fn new() -> Self
```

---

### RouteParams::get()

**Purpose**: Get parameter value

**Signature**:
```rust
pub fn get(&self, key: &str) -> Option<&str>
```

**Usage**:
```rust
if let Some(id) = params.get("id") {
    println!("User ID: {}", id);
}
```

---

### RouteParams::set()

**Purpose**: Set parameter value (for programmatic navigation)

**Signature**:
```rust
pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>)
```

**Usage**:
```rust
let mut params = RouteParams::new();
params.set("id", "123");
params.set("tab", "profile");
```

---

### RouteParams::iter()

**Purpose**: Iterate all params

**Signature**:
```rust
pub fn iter(&self) -> impl Iterator<Item = (&str, &str)>
```

**Usage**:
```rust
for (key, value) in params.iter() {
    println!("{} = {}", key, value);
}
```

---

## Global Router Setup

### Router Initialization

**Purpose**: Configure and register routes at app startup

**Usage Pattern**:
```rust
fn main() {
    gpui::App::new().run(|cx| {
        // Define route tree
        let routes = vec![
            Route::component("/", HomePage::new),
            Route::new("/dashboard", dashboard_layout)
                .children(vec![
                    Route::new("", overview_page), // Index
                    Route::component("analytics", AnalyticsPage::new),
                ]),
            Route::component_with_params("/user/:id", |params| {
                UserPage::new(params.get("id").unwrap().to_string())
            }),
        ];
        
        // Initialize router
        let router = Router::new(routes);
        cx.set_global(router);
        
        // Navigate to initial route
        Navigator::replace(cx, "/");
        
        // Open app window
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| AppRoot::new())
        });
    });
}
```

**Guarantees**:
- Routes validated at initialization (circular dependency check)
- Router available globally via `cx.global::<Router>()`
- All routes registered before first navigation

---

## Error Handling

### Error Boundary in Outlets

**Behavior**:
When a route component panics during construction or render:
1. Outlet catches the panic (via error boundary)
2. Displays error UI within outlet area
3. Parent layout remains functional
4. User can navigate away to recover

**Error UI Content**:
```rust
div()
    .child("⚠️ Route Error")
    .child(format!("Failed to render: {}", error_message))
    .child("Please try refreshing or contact support")
```

**Developer API**:
```rust
// Optional: Custom error page per route
Route::new("/dashboard", dashboard_layout)
    .error_page(|error| custom_error_ui(error))
```

---

### Not Found Handling

**Behavior**:
When no route matches the current path:
1. Router searches for configured not-found route
2. If found, renders not-found component
3. If not configured, renders default 404 page

**Configuration**:
```rust
let router = Router::new(routes)
    .not_found(Route::component("/404", NotFoundPage::new));
```

**Default 404 Page**:
Production-ready styled page with:
- "404 - Page Not Found" message
- Link to home page
- Search functionality (optional)

---

## Performance Guarantees

### Route Resolution

- **Time**: O(depth × siblings) where depth <5, siblings <10 typical
- **Target**: <1ms for typical app (100 routes, 5 levels deep)

### Component Caching

- **Hit rate**: >80% for typical navigation patterns (back/forward, tab switching)
- **Eviction overhead**: <5ms when cache full

### Navigation

- **Latency**: <16ms from Navigator call to first paint (60fps target)
- **Cancellation**: In-flight navigation cancelled within single frame

### Memory

- **Cache overhead**: <100KB for default 10-route capacity
- **Route tree**: O(routes) × ~1KB per route typical

---

## Thread Safety

All public APIs are thread-safe:
- `Route` uses `Arc` for immutable sharing
- `Navigator` methods accept `&mut App` (GPUI enforces single-threaded access)
- Route builders are `Send + Sync`
- No interior mutability in hot paths

---

## Breaking Change Policy

Following semantic versioning:

**MAJOR** (e.g., 1.0 → 2.0):
- Signature changes (e.g., new required parameter)
- Behavior changes (e.g., cache disabled by default)
- Removed public APIs

**MINOR** (e.g., 1.0 → 1.1):
- New public APIs
- New optional features
- Deprecations (with migration guide)

**PATCH** (e.g., 1.0.0 → 1.0.1):
- Bug fixes
- Documentation improvements
- Internal refactors (no API changes)

---

## Feature Flag Matrix

| Feature | APIs Added | Default |
|---------|-----------|---------|
| `cache` | `Navigator::clear_cache()`, `Navigator::clear_all_cache()` | Enabled |
| `transition` | `Route::transition()`, `TransitionConfig` API | Enabled |
| `guard` | `Route::guard()`, guard hooks | Disabled |
| `middleware` | `Route::middleware()`, middleware API | Disabled |
| `tracing` | Structured log events | Disabled (use `log` instead) |

---

## Deprecation Example

When deprecating an API:

```rust
#[deprecated(since = "1.2.0", note = "Use `Navigator::push()` instead")]
pub fn navigate_to(cx: &mut App, path: &str) {
    Navigator::push(cx, path);
}
```

Deprecations remain for 1 MAJOR version before removal.

---

## Next Steps

- [x] Core API contracts defined
- [ ] Quickstart guide with examples
- [ ] Implementation plan with constitution check
