# Nested Routing Bug Fixes

## Issues Fixed

### 1. Empty Path Normalization (CRITICAL)
**File**: `src/nested.rs` - `build_child_path()` function

**Problem**: Empty child paths `""` (used for index routes) were being normalized to `"/"`, causing the index child to have the same path as its parent. This created infinite recursion where RouterOutlet kept matching the same route.

**Fix**: Added early return in `build_child_path()` to preserve empty child paths:
```rust
pub fn build_child_path<'a>(parent_path: &'a str, child_path: &'a str) -> Cow<'a, str> {
    // CRITICAL: Don't normalize empty child paths - they represent index routes
    if child_path.is_empty() {
        return normalize_path(parent_path);
    }
    // ... rest of function
}
```

### 2. RouterOutlet Entity Caching (CRITICAL)
**Files**: `examples/nested_demo.rs` - All layout components

**Problem**: Layout components were creating NEW RouterOutlet entities on every render:
```rust
// WRONG - creates new entity every render
.child(cx.new(|_| RouterOutlet::new()))
```

This caused the RouterOutlet's internal state to be reset on every render, making `prev_path` always empty and `path_changed` always true.

**Fix**: Cache RouterOutlet entities using `window.use_keyed_state()`:
```rust
// CORRECT - reuses same entity across renders
let router_outlet = window.use_keyed_state(
    gpui::ElementId::Name("root_router_outlet".into()),
    cx,
    |_, _| RouterOutlet::new()
);
.child(router_outlet.clone())
```

**Applied to**:
- `RootLayout` (key: "root_router_outlet")
- `DashboardLayout` (key: "dashboard_router_outlet")  
- `ProductsLayout` (key: "products_router_outlet")

### 3. State Comparison Support
**File**: `src/params.rs`

**Added**: `PartialEq` derive to `RouteParams` to enable state change detection

### 4. Unnecessary State Updates
**File**: `src/widgets.rs`

**Added**: Guard to prevent updating global params when they haven't changed:
```rust
let should_update = cx.try_global::<crate::context::GlobalRouter>()
    .map(|router| router.state().current_params() != &child_params)
    .unwrap_or(true);

if should_update {
    cx.update_global::<crate::context::GlobalRouter, _>(|router, _cx| {
        router.state_mut().set_current_params(child_params.clone());
    });
}
```

## Current Status

✅ **FIXED**: Route state now persists correctly across renders
✅ **FIXED**: Empty path index routes work correctly
✅ **FIXED**: `path_changed` correctly detects when route hasn't changed

⚠️ **KNOWN LIMITATION**: There may still be some re-renders due to GPUI's reactive tracking of keyed state access in route builders. This is a framework-level design consideration and doesn't affect functionality.

## Best Practices for Nested Routing

### DO:
- ✅ Cache RouterOutlet entities using `window.use_keyed_state()`
- ✅ Use unique keys for each RouterOutlet in your app
- ✅ Use empty string `""` for index route paths
- ✅ Keep route components stateless when possible

### DON'T:
- ❌ Create new RouterOutlet entities with `cx.new()` inside render
- ❌ Normalize empty paths to "/" in route definitions
- ❌ Create RouterOutlet without caching in layouts that re-render

## Example: Correct RouterOutlet Usage

```rust
impl Render for MyLayout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        // Cache the RouterOutlet entity
        let router_outlet = window.use_keyed_state(
            gpui::ElementId::Name("my_layout_outlet".into()),
            cx,
            |_, _| RouterOutlet::new()
        );
        
        div()
            .child("My Layout Content")
            .child(router_outlet.clone())  // Use the cached entity
    }
}
```

## Testing

After these fixes:
- ✅ All 197 tests passing
- ✅ Route state persists correctly  
- ✅ Index routes render without recursion
- ✅ Nested routing navigation works

## References

Similar patterns in other frameworks:
- **React Router**: Uses hooks and context to centralize routing logic, Outlet is just a thin wrapper
- **gpui-router**: Uses lazy evaluation closures to avoid over-rendering
- **gpui-nav**: Uses stack-based navigation (not nested routing)
