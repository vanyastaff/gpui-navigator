# Quickstart: Nested Routing Improvements

**Feature**: 001-nested-routing  
**Date**: 2026-01-28  
**Audience**: Developers implementing the nested routing fixes

## Overview

This guide walks through implementing the nested routing improvements from specification to testing. Follow these steps in order to ensure all bugs are fixed and optimizations applied correctly.

## Prerequisites

- Rust 1.75+ installed
- GPUI 0.2.x dependency in Cargo.toml
- Familiarity with the current `src/nested.rs` implementation
- Read `research.md` to understand bugs and proposed solutions

## Phase 1: Critical Bug Fixes (P1)

### Step 1.1: Fix Parameter Extraction (BUG-002, BUG-003, BUG-004)

**Goal**: Extract parameters recursively and strip constraints correctly

**Files to modify**:
- `src/nested.rs` (resolve_child_route function)
- Add helper function `extract_param_name`

**Implementation**:

1. Add parameter name extraction helper:
```rust
/// Extract parameter name from route segment, stripping constraints
/// Example: ":id{uuid}" → "id"
pub(crate) fn extract_param_name(segment: &str) -> &str {
    let param = segment.strip_prefix(':').unwrap_or(segment);
    if let Some(pos) = param.find('{') {
        &param[..pos]
    } else {
        param
    }
}
```

2. Modify parameter extraction in `resolve_child_route`:
```rust
// OLD:
if child_path.starts_with(':') {
    let param_name = child_path.trim_start_matches(':');
    combined_params.insert(param_name.to_string(), first_segment.to_string());
}

// NEW:
if child_path.starts_with(':') {
    let param_name = extract_param_name(child_path);
    combined_params.insert(param_name.to_string(), first_segment.to_string());
}
```

3. Add recursive resolution for remaining segments:
```rust
// After matching first segment:
if child_path == first_segment || child_path.starts_with(':') {
    let mut combined_params = parent_params.clone();
    
    if child_path.starts_with(':') {
        let param_name = extract_param_name(child_path);
        combined_params.insert(param_name.to_string(), first_segment.to_string());
    }

    // NEW: Check for remaining segments
    if segments.len() > 1 {
        // More segments exist, recurse into child's children
        let remaining_segments = &segments[1..];
        let child_remaining = remaining_segments.join("/");
        
        return resolve_child_route(
            &child,
            &format!("/{}", child_remaining),  // Reconstruct path
            &combined_params,
            outlet_name,
        );
    }

    return Some((Arc::clone(child), combined_params));
}
```

**Test**:
```rust
#[test]
fn test_recursive_parameter_extraction() {
    let parent = Route::new("/users/:userId", |_, _| div())
        .children(vec![
            Route::new("posts/:postId", |_, _| div()),
        ]);
    
    let params = RouteParams::new();
    let resolved = resolve_child_route(&parent, "/users/123/posts/456", &params, None);
    
    assert!(resolved.is_some());
    let (_, params) = resolved.unwrap();
    assert_eq!(params.get("userId"), Some(&"123".to_string()));
    assert_eq!(params.get("postId"), Some(&"456".to_string()));
}
```

---

### Step 1.2: Fix Double Normalization (BUG-001)

**Goal**: Remove redundant path normalization

**Files to modify**:
- `src/nested.rs` (resolve_child_route function, line 69)

**Implementation**:

```rust
// OLD (line 69):
if !current_path_normalized.starts_with(parent_path_normalized.trim_start_matches('/')) {
    return None;
}

// NEW:
if !current_path_normalized.starts_with(parent_path_normalized) {
    return None;
}
```

**Explanation**: `parent_path_normalized` is already normalized at line 65, don't apply trim again.

**Test**:
```rust
#[test]
fn test_root_path_matching() {
    let parent = Route::new("/", |_, _| div())
        .children(vec![
            Route::new("dashboard", |_, _| div()),
        ]);
    
    let params = RouteParams::new();
    let resolved = resolve_child_route(&parent, "/dashboard", &params, None);
    
    assert!(resolved.is_some());
}
```

---

### Step 1.3: Fix Parent Parameters Lost (BUG-004)

**Goal**: Pass parent route parameters to child resolution

**Files to modify**:
- `src/widgets.rs` (RouterOutlet::render method)

**Current Code** (src/widgets.rs:210-211):
```rust
let route_params = crate::RouteParams::new();  // ← Empty!
let resolved = resolve_child_route(
    parent_route,
    &current_path,
    &route_params,
    self.name.as_deref(),
);
```

**Implementation**:

The parent route parameters need to be extracted from the parent route's own path match. This requires:

1. Store matched parameters in RouterState when a route is matched
2. Retrieve those parameters in RouterOutlet::render

**Approach**:

Modify RouterState to store current route parameters:
```rust
// In src/state.rs (RouterState struct):
pub struct RouterState {
    pub routes: Vec<Arc<Route>>,
    pub current_path: Option<String>,
    pub history: Vec<String>,
    pub history_index: usize,
    pub default_pages: DefaultPages,
    pub current_params: RouteParams,  // NEW: Store current matched params
}
```

Then in `src/widgets.rs`:
```rust
// In RouterOutlet::render, retrieve parent params:
let parent_params = if let Some(router_state) = router_state {
    &router_state.current_params
} else {
    &RouteParams::new()
};

let resolved = resolve_child_route(
    parent_route,
    &current_path,
    parent_params,  // Pass parent params instead of empty
    self.name.as_deref(),
);
```

**Test**:
```rust
#[test]
fn test_parent_params_inherited() {
    // Set up parent route with parameter
    let parent = Route::new("/workspace/:wid", |_, _| div())
        .children(vec![
            Route::new("projects", |_, _| div()),
        ]);
    
    // Simulate navigation to /workspace/abc/projects
    // Parent should extract {wid: "abc"}
    // Child resolution should receive {wid: "abc"}
    
    let parent_params = RouteParams::from([("wid".to_string(), "abc".to_string())]);
    let resolved = resolve_child_route(&parent, "/workspace/abc/projects", &parent_params, None);
    
    assert!(resolved.is_some());
    let (_, params) = resolved.unwrap();
    assert_eq!(params.get("wid"), Some(&"abc".to_string()));
}
```

---

## Phase 2: Path Normalization (P2)

### Step 2.1: Create Path Normalization Helper

**Goal**: Consistent path normalization across module

**Files to add**:
- Helper function in `src/nested.rs`

**Implementation**:

```rust
use std::borrow::Cow;

/// Normalize a route path to canonical form
/// - Removes leading and trailing slashes
/// - Root path "/" becomes empty string ""
/// - Returns Cow to avoid allocations when already normalized
pub(crate) fn normalize_path(path: &str) -> Cow<'_, str> {
    let trimmed = path.trim_matches('/');
    if trimmed == path {
        Cow::Borrowed(path)  // Already normalized
    } else {
        Cow::Owned(trimmed.to_string())  // Modified
    }
}
```

**Test**:
```rust
#[test]
fn test_normalize_path() {
    assert_eq!(normalize_path("/dashboard"), "dashboard");
    assert_eq!(normalize_path("dashboard/"), "dashboard");
    assert_eq!(normalize_path("/dashboard/"), "dashboard");
    assert_eq!(normalize_path("dashboard"), "dashboard");  // Already normalized
    assert_eq!(normalize_path("/"), "");  // Root
    assert_eq!(normalize_path(""), "");  // Empty
}
```

### Step 2.2: Apply Normalization Consistently

**Goal**: Replace all ad-hoc `trim_start_matches`/`trim_end_matches` with `normalize_path`

**Files to modify**:
- `src/nested.rs` (resolve_child_route, find_index_route, build_child_path)

**Replacements**:

```rust
// In resolve_child_route:
// OLD:
let parent_path_normalized = parent_path.trim_end_matches('/');
let current_path_normalized = current_path.trim_start_matches('/');

// NEW:
let parent_path_normalized = normalize_path(parent_path);
let current_path_normalized = normalize_path(current_path);
```

```rust
// In find_index_route:
// OLD:
let child_path = child.config.path
    .trim_start_matches('/')
    .trim_end_matches('/');

// NEW:
let child_path = normalize_path(&child.config.path);
```

```rust
// In build_child_path:
// OLD:
let parent = parent_path.trim_end_matches('/');
let child = child_path.trim_start_matches('/').trim_end_matches('/');

// NEW:
let parent = normalize_path(parent_path);
let child = normalize_path(child_path);
```

---

## Phase 3: Performance Optimizations (P3)

### Step 3.1: Reduce Allocations (OPT-001)

**Goal**: Avoid Vec allocation for single-segment paths

**Files to modify**:
- `src/nested.rs` (resolve_child_route segment splitting)

**Implementation**:

```rust
// OLD:
let segments: Vec<&str> = remaining.split('/').filter(|s| !s.is_empty()).collect();
if segments.is_empty() {
    return find_index_route(children, parent_params.clone());
}
let first_segment = segments[0];

// NEW:
// Fast path: single segment (no '/')
let (first_segment, remaining_segments) = if !remaining.contains('/') {
    (remaining, &[] as &[&str])
} else {
    // Slow path: multiple segments
    let segments: Vec<&str> = remaining.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return find_index_route(children, parent_params.clone());
    }
    (segments[0], &segments[1..])
};
```

**Benefit**: Eliminates Vec allocation for ~70% of cases (shallow nesting).

---

### Step 3.2: Prefer Static Routes (OPT-002)

**Goal**: Match static routes before parameter routes

**Files to modify**:
- `src/nested.rs` (resolve_child_route matching loop)

**Implementation**:

```rust
// OLD: Single pass, first match wins
for child in children {
    if child_path == first_segment || child_path.starts_with(':') {
        // ... return
    }
}

// NEW: Two-pass matching
// Pass 1: Exact (static) matches
for child in children {
    let child_path = normalize_path(&child.config.path);
    if child_path == first_segment {
        // ... extract params and return
    }
}

// Pass 2: Parameter matches
for child in children {
    let child_path = normalize_path(&child.config.path);
    if child_path.starts_with(':') {
        let param_name = extract_param_name(&child_path);
        // ... extract param and return
    }
}

None
```

**Benefit**: Correct semantics + predictable matching.

---

### Step 3.3: Add Resolution Caching (BUG-005)

**Goal**: Cache route resolution results (feature flag)

**Files to modify**:
- `src/nested.rs` (add caching logic)
- `Cargo.toml` (cache feature already exists)

**Implementation**:

```rust
#[cfg(feature = "cache")]
use lru::LruCache;
#[cfg(feature = "cache")]
use std::cell::RefCell;
#[cfg(feature = "cache")]
use std::num::NonZeroUsize;

#[cfg(feature = "cache")]
thread_local! {
    static RESOLUTION_CACHE: RefCell<LruCache<ResolutionKey, ResolvedChildRoute>> =
        RefCell::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

#[cfg(feature = "cache")]
#[derive(Hash, Eq, PartialEq, Clone)]
struct ResolutionKey {
    parent_path: String,
    current_path: String,
    outlet_name: Option<String>,
}

pub fn resolve_child_route(
    parent_route: &Arc<Route>,
    current_path: &str,
    parent_params: &RouteParams,
    outlet_name: Option<&str>,
) -> Option<ResolvedChildRoute> {
    #[cfg(feature = "cache")]
    {
        let key = ResolutionKey {
            parent_path: parent_route.config.path.clone(),
            current_path: current_path.to_string(),
            outlet_name: outlet_name.map(|s| s.to_string()),
        };
        
        if let Some(cached) = RESOLUTION_CACHE.with(|c| c.borrow_mut().get(&key).cloned()) {
            trace_log!("Cache HIT for {:?}", key);
            return Some(cached);
        }
    }

    // ... existing resolution logic ...

    #[cfg(feature = "cache")]
    if let Some(result) = &result {
        RESOLUTION_CACHE.with(|c| {
            c.borrow_mut().put(key, result.clone());
        });
    }

    result
}
```

**Test**:
```rust
#[cfg(feature = "cache")]
#[test]
fn test_resolution_caching() {
    let parent = Route::new("/dashboard", |_, _| div())
        .children(vec![
            Route::new("settings", |_, _| div()),
        ]);
    
    let params = RouteParams::new();
    
    // First call - cache miss
    let result1 = resolve_child_route(&parent, "/dashboard/settings", &params, None);
    assert!(result1.is_some());
    
    // Second call - cache hit
    let result2 = resolve_child_route(&parent, "/dashboard/settings", &params, None);
    assert!(result2.is_some());
    
    // Results should be identical
    assert_eq!(result1.unwrap().0.config.path, result2.unwrap().0.config.path);
}
```

---

## Phase 4: Index Route Handling (P2)

### Step 4.1: Enforce Single Index Route

**Goal**: Validate only one index route exists per children vec

**Files to modify**:
- `src/route.rs` (Route::children validation)

**Implementation**:

```rust
impl Route {
    pub fn children(mut self, children: Vec<Route>) -> Self {
        // Validate single index route
        let index_routes: Vec<_> = children
            .iter()
            .filter(|r| {
                let path = normalize_path(&r.config.path);
                path.is_empty() || path == "index"
            })
            .collect();
        
        if index_routes.len() > 1 {
            panic!(
                "Multiple index routes found in children of '{}': found {} routes with empty or 'index' path. Only one index route is allowed.",
                self.config.path,
                index_routes.len()
            );
        }
        
        // Validate relative paths
        for child in &children {
            if child.config.path.starts_with('/') && child.config.path != "/" {
                warn_log!(
                    "Child route '{}' has absolute path (starts with '/'). Child routes should be relative. This will be rejected in v0.2.0.",
                    child.config.path
                );
            }
        }
        
        self.config.children = children.into_iter().map(Arc::new).collect();
        self
    }
}
```

**Test**:
```rust
#[test]
#[should_panic(expected = "Multiple index routes")]
fn test_reject_multiple_index_routes() {
    Route::new("/parent", |_, _| div())
        .children(vec![
            Route::new("", |_, _| div()),      // Index 1
            Route::new("index", |_, _| div()), // Index 2
        ]);
}
```

---

### Step 4.2: Handle Missing Index Route

**Goal**: Explicit behavior when no index route exists

**Files to modify**:
- `src/nested.rs` (find_index_route function)

**Implementation**:

```rust
fn find_index_route(children: &[Arc<Route>], params: RouteParams) -> Option<ResolvedChildRoute> {
    trace_log!("find_index_route: searching {} children", children.len());

    for child in children {
        let child_path = normalize_path(&child.config.path);
        
        if child_path.is_empty() || child_path == "index" {
            trace_log!("  ✓ found index route: '{}'", child.config.path);
            return Some((Arc::clone(child), params));
        }
    }

    // NEW: Log warning when no index route found
    warn_log!("No index route found for parent with {} children. Consider adding a child with empty path (\"\") to define default route.", children.len());
    None
}
```

---

## Phase 5: Testing & Documentation (P1)

### Step 5.1: Unit Tests

**Files to create/modify**:
- `tests/unit/nested_resolution_tests.rs` (new file)

**Test Coverage**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Path normalization tests
    #[test]
    fn test_normalize_empty_path() { /* ... */ }
    
    #[test]
    fn test_normalize_root_path() { /* ... */ }
    
    #[test]
    fn test_normalize_trailing_slash() { /* ... */ }

    // Segment matching tests
    #[test]
    fn test_static_segment_match() { /* ... */ }
    
    #[test]
    fn test_parameter_segment_match() { /* ... */ }
    
    #[test]
    fn test_static_preferred_over_parameter() { /* ... */ }

    // Parameter extraction tests
    #[test]
    fn test_single_parameter_extraction() { /* ... */ }
    
    #[test]
    fn test_parameter_constraint_stripped() { /* ... */ }
    
    #[test]
    fn test_recursive_parameter_extraction() { /* ... */ }
    
    #[test]
    fn test_parameter_inheritance() { /* ... */ }

    // Index route tests
    #[test]
    fn test_index_route_empty_path() { /* ... */ }
    
    #[test]
    fn test_index_route_explicit() { /* ... */ }
    
    #[test]
    fn test_no_index_route_returns_none() { /* ... */ }

    // Named outlet tests
    #[test]
    fn test_named_outlet_resolution() { /* ... */ }
    
    #[test]
    fn test_missing_named_outlet() { /* ... */ }

    // Performance tests
    #[cfg(feature = "cache")]
    #[test]
    fn test_caching_behavior() { /* ... */ }
}
```

**Target**: 80%+ code coverage for src/nested.rs

---

### Step 5.2: Integration Tests

**Files to create/modify**:
- `tests/integration/nested_navigation_tests.rs` (new file)

**Test Scenarios**:

```rust
#[gpui::test]
async fn test_shallow_nesting() {
    // /dashboard -> /dashboard/overview
    // Verify child renders correctly
}

#[gpui::test]
async fn test_deep_nesting() {
    // /workspace/:wid/projects/:pid/tasks/:tid
    // Verify parameters extracted at all levels
}

#[gpui::test]
async fn test_parameter_inheritance() {
    // Parent param accessible to child
}

#[gpui::test]
async fn test_named_outlets() {
    // Two outlets render independently
}

#[gpui::test]
async fn test_index_route_navigation() {
    // Navigate to parent exact path, index child renders
}
```

---

### Step 5.3: Update Examples

**Files to modify**:
- `examples/nested_demo.rs` (update with new scenarios)

**Add**:
1. Parameter inheritance example
2. Deep nesting example (3+ levels)
3. Named outlets example
4. Edge case handling (no index route)

---

### Step 5.4: Documentation

**Files to modify**:
- `src/nested.rs` (add rustdoc comments)

**Add rustdoc for**:
- `resolve_child_route` function
- `find_index_route` function
- `build_child_path` function
- `normalize_path` helper
- `extract_param_name` helper

**Example**:
```rust
/// Resolves a child route within a nested parent route hierarchy.
///
/// # Arguments
///
/// * `parent_route` - The parent route containing children
/// * `current_path` - The full navigation path (e.g., "/dashboard/settings")
/// * `parent_params` - Parameters extracted from parent route
/// * `outlet_name` - Optional named outlet (None for default outlet)
///
/// # Returns
///
/// Returns `Some((child_route, merged_params))` if a matching child is found,
/// or `None` if no child matches the current path.
///
/// # Examples
///
/// ```
/// use gpui_navigator::*;
///
/// let parent = Route::new("/dashboard", |_, _| div())
///     .children(vec![
///         Route::new("settings", |_, _| div()),
///     ]);
///
/// let params = RouteParams::new();
/// let resolved = resolve_child_route(&parent, "/dashboard/settings", &params, None);
/// assert!(resolved.is_some());
/// ```
pub fn resolve_child_route(...) -> Option<ResolvedChildRoute> {
    // ...
}
```

---

## Validation Checklist

Before submitting PR:

- [ ] All unit tests pass (`cargo test --lib`)
- [ ] All integration tests pass (`cargo test --test integration`)
- [ ] Examples run without errors (`cargo run --example nested_demo`)
- [ ] Code coverage ≥80% for src/nested.rs
- [ ] No clippy warnings (`cargo clippy --all-features`)
- [ ] Documentation builds (`cargo doc --all-features`)
- [ ] Performance benchmark <1ms (`cargo bench nested_resolution`)
- [ ] Constitution check passes (see plan.md)

---

## Performance Benchmarking

**Setup**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_resolution(c: &mut Criterion) {
    let parent = create_complex_route_tree();  // 50 routes, 5 levels
    let params = RouteParams::new();
    
    c.bench_function("resolve_deep_nested", |b| {
        b.iter(|| {
            resolve_child_route(
                black_box(&parent),
                black_box("/level1/level2/level3/level4/level5"),
                black_box(&params),
                black_box(None),
            )
        });
    });
}

criterion_group!(benches, benchmark_resolution);
criterion_main!(benches);
```

**Target**: <1ms per resolution

---

## Troubleshooting

### Issue: Tests fail with "parameter not found"

**Cause**: Parent parameters not passed to child resolution

**Fix**: Verify RouterState stores current_params and RouterOutlet retrieves them

---

### Issue: Index route not rendering

**Cause**: No child with empty path "" or "index"

**Fix**: Add explicit index route or handle None return gracefully

---

### Issue: Cache not working

**Cause**: Feature flag not enabled

**Fix**: Add `features = ["cache"]` in Cargo.toml or run with `--features cache`

---

## Next Steps

After completing this quickstart:

1. Run `/speckit.tasks` to generate detailed task breakdown
2. Implement tasks in priority order (P1 → P2 → P3)
3. Review against constitution principles
4. Submit PR with tests and documentation

---

## Constitution Alignment

This implementation follows:

- **Principle IV**: Nested Routing Excellence (comprehensive fixes)
- **Principle V**: Type Safety & Rust Idioms (Arc, Cow, no unsafe)
- **Principle VII**: Test-First for Complex Features (80%+ coverage)

All changes are backward compatible (no breaking API changes).
