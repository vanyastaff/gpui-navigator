# Quickstart Guide: Nested Routing

**Feature**: 001-nested-routing-redesign  
**Audience**: Developers implementing the new architecture  
**Time**: 30 minutes to understand, 2-3 days to implement

## Architecture Overview

```text
┌─────────────────────────────────────────────────┐
│ Application (GPUI App)                          │
│  ├─ GlobalRouter (Arc<RouterState>)             │
│  │   ├─ Route Tree (Vec<Arc<Route>>)            │
│  │   ├─ Current Path & Params                   │
│  │   └─ ComponentCache (LRU, optional)          │
│  └─ Window                                      │
│      └─ AppRoot Component                       │
│          └─ RouterOutlet (renders matched route)│
│              └─ Page Component (from builder)   │
│                  └─ Nested RouterOutlet         │
│                      └─ Child Component         │
└─────────────────────────────────────────────────┘
```

**Key Principles** (from research):
1. **Segment-based matching**: `/dashboard/analytics` → ["dashboard", "analytics"]
2. **Two-phase rendering**: Resolve route → Build component (prevents double-borrow)
3. **Hierarchical outlets**: Each outlet consumes path segments (prevents recursion)
4. **LRU caching**: Stateful components cached (default 10 routes)
5. **Cancellation-based navigation**: Latest navigation wins

---

## Phase 1: Core Routing (No Nesting)

### Step 1.1: Route Tree Structure

**File**: `src/route.rs`

```rust
use std::sync::Arc;

pub struct Route {
    path: String,                    // "dashboard", ":id", ""
    builder: Option<RouteBuilder>,   // Fn(window, cx, params) -> AnyElement
    children: Vec<Arc<Route>>,       // Nested routes
    name: Option<String>,            // For named navigation
    
    #[cfg(feature = "transition")]
    transition: TransitionConfig,
}

pub type RouteBuilder = Arc<dyn Fn(&mut Window, &mut App, &RouteParams) -> AnyElement + Send + Sync>;

impl Route {
    pub fn new<F>(path: impl Into<String>, builder: F) -> Self
    where
        F: Fn(&mut Window, &mut App, &RouteParams) -> AnyElement + Send + Sync + 'static
    {
        Self {
            path: path.into(),
            builder: Some(Arc::new(builder)),
            children: Vec::new(),
            name: None,
            #[cfg(feature = "transition")]
            transition: TransitionConfig::default(),
        }
    }
    
    pub fn children(mut self, children: Vec<Route>) -> Self {
        self.children = children.into_iter().map(Arc::new).collect();
        self
    }
    
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}
```

**Why**: Immutable Arc<Route> tree is thread-safe, cloneable, shared across app

---

### Step 1.2: Path Matching (Segment-Based)

**File**: `src/matching.rs`

```rust
pub struct RouteMatch<'a> {
    pub route: &'a Arc<Route>,
    pub params: RouteParams,
}

/// Match path against route pattern
/// Example: match_path("/user/123", "/user/:id") → Some({id: "123"})
pub fn match_path<'a>(
    path: &str,
    route: &'a Arc<Route>,
) -> Option<RouteMatch<'a>> {
    let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let route_segments: Vec<&str> = route.path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_segments.len() != route_segments.len() {
        return None;
    }
    
    let mut params = RouteParams::new();
    
    for (path_seg, route_seg) in path_segments.iter().zip(route_segments.iter()) {
        if route_seg.starts_with(':') {
            // Parameter segment
            let param_name = &route_seg[1..];
            params.set(param_name, *path_seg);
        } else if path_seg != route_seg {
            // Literal mismatch
            return None;
        }
    }
    
    Some(RouteMatch { route, params })
}
```

**Why**: Segment-based is O(n) where n=depth, simpler than regex, sufficient for 99% of apps

---

### Step 1.3: RouterState with Navigation Counter

**File**: `src/state.rs`

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct RouterState {
    current_path: String,
    current_params: RouteParams,
    current_route: Option<Arc<Route>>,
    routes: Vec<Arc<Route>>,
    navigation_id: AtomicUsize,  // Monotonic counter
    
    #[cfg(feature = "cache")]
    component_cache: ComponentCache,
}

impl RouterState {
    pub fn new(routes: Vec<Route>) -> Self {
        Self {
            current_path: String::from("/"),
            current_params: RouteParams::new(),
            current_route: None,
            routes: routes.into_iter().map(Arc::new).collect(),
            navigation_id: AtomicUsize::new(0),
            #[cfg(feature = "cache")]
            component_cache: ComponentCache::new(10), // Default capacity
        }
    }
    
    pub fn navigate(&mut self, path: String) -> Result<(), NavigationError> {
        // Cancel any in-flight navigation (just overwrite)
        let nav_id = self.navigation_id.fetch_add(1, Ordering::SeqCst);
        
        // Resolve route
        let matched = self.find_route(&path)?;
        
        // Update state
        self.current_path = path;
        self.current_params = matched.params;
        self.current_route = Some(matched.route.clone());
        
        Ok(())
    }
    
    fn find_route(&self, path: &str) -> Result<RouteMatch, NavigationError> {
        for route in &self.routes {
            if let Some(matched) = match_path(path, route) {
                return Ok(matched);
            }
        }
        Err(NavigationError::NotFound)
    }
}
```

**Why**: Navigation ID prevents stale navigation completion, LRU cache optional for phase 1

---

### Step 1.4: Navigator API

**File**: `src/navigator.rs`

```rust
pub struct Navigator;

impl Navigator {
    pub fn push(cx: &mut App, path: impl Into<String>) {
        let path = path.into();
        
        cx.update_global::<RouterState, _>(|router, _cx| {
            router.navigate(path).ok(); // TODO: handle errors
        });
        
        cx.notify(); // Mark window dirty
    }
    
    pub fn replace(cx: &mut App, path: impl Into<String>) {
        // Same as push for now (history comes later)
        Self::push(cx, path);
    }
}
```

**Why**: Simple API, defers to RouterState for logic, triggers GPUI re-render via notify()

---

### Step 1.5: RouterOutlet (Simple Version)

**File**: `src/widgets.rs`

```rust
pub struct RouterOutlet {
    name: Option<String>,
    state: RefCell<OutletState>,
}

struct OutletState {
    current_path: String,
    animation_counter: u32,
}

impl RouterOutlet {
    pub fn new() -> Self {
        Self {
            name: None,
            state: RefCell::new(OutletState {
                current_path: String::new(),
                animation_counter: 0,
            }),
        }
    }
}

impl Render for RouterOutlet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // PHASE 1: Resolve (immutable borrow)
        let (route, params) = {
            let router = cx.try_global::<RouterState>();
            let Some(router) = router else {
                return div().child("No router configured").into_any_element();
            };
            
            let route = router.current_route.clone();
            let params = router.current_params.clone();
            
            (route, params)
        }; // Drop router borrow here
        
        // PHASE 2: Build (mutable context)
        if let Some(route) = route {
            if let Some(builder) = &route.builder {
                return builder(window, cx, &params);
            }
        }
        
        div().child("No route matched").into_any_element()
    }
}
```

**Why**: Two-phase prevents double-borrow, simple for phase 1, no nesting yet

---

## Phase 2: Nested Routing

### Step 2.1: Hierarchical Route Resolution

**File**: `src/nested.rs`

```rust
pub fn resolve_child_route<'a>(
    parent: &'a Arc<Route>,
    full_path: &str,
    parent_params: &RouteParams,
) -> Option<RouteMatch<'a>> {
    // Extract remaining path after parent
    let parent_segments: Vec<&str> = parent.path.split('/').filter(|s| !s.is_empty()).collect();
    let full_segments: Vec<&str> = full_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if full_segments.len() <= parent_segments.len() {
        // No child path, check for index route
        return find_index_route(parent, parent_params);
    }
    
    // Remaining segments for child
    let child_segments = &full_segments[parent_segments.len()..];
    let child_path = format!("/{}", child_segments.join("/"));
    
    // Match against parent's children
    for child in &parent.children {
        if let Some(matched) = match_path(&child_path, child) {
            // Merge parent params into child params
            let merged_params = merge_params(parent_params, &matched.params);
            return Some(RouteMatch {
                route: matched.route,
                params: merged_params,
            });
        }
    }
    
    None
}

fn find_index_route<'a>(
    parent: &'a Arc<Route>,
    parent_params: &RouteParams,
) -> Option<RouteMatch<'a>> {
    for child in &parent.children {
        if child.path.is_empty() {
            return Some(RouteMatch {
                route: child,
                params: parent_params.clone(),
            });
        }
    }
    None
}

fn merge_params(parent: &RouteParams, child: &RouteParams) -> RouteParams {
    let mut merged = parent.clone();
    for (key, value) in child.iter() {
        merged.set(key, value); // Child overrides parent on collision
    }
    merged
}
```

**Why**: Hierarchical resolution consumes segments, preventing parent re-match (loop prevention)

---

### Step 2.2: RouterOutlet with Nested Resolution

**File**: `src/widgets.rs` (update)

```rust
impl Render for RouterOutlet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // PHASE 1: Resolve child route
        let (child_route, child_params, current_path) = {
            let router = cx.try_global::<RouterState>();
            let Some(router) = router else {
                return div().child("No router").into_any_element();
            };
            
            let current_path = router.current_path.clone();
            
            // Find parent route that contains this outlet
            let parent_route = find_parent_for_outlet(&router, &current_path, self.name.as_deref());
            
            let Some(parent) = parent_route else {
                // No parent found, render nothing (graceful degradation)
                return div().into_any_element();
            };
            
            // Resolve child within parent
            let child_match = resolve_child_route(&parent.route, &current_path, &parent.params);
            
            let Some(child) = child_match else {
                // No child matched, render nothing
                return div().into_any_element();
            };
            
            (child.route.clone(), child.params, current_path)
        }; // Drop router borrow
        
        // Check if path changed
        let path_changed = {
            let state = self.state.borrow();
            state.current_path != current_path
        };
        
        if path_changed {
            // Update internal state
            let mut state = self.state.borrow_mut();
            state.current_path = current_path;
            state.animation_counter = state.animation_counter.wrapping_add(1);
        }
        
        // PHASE 2: Build child component
        if let Some(builder) = &child_route.builder {
            builder(window, cx, &child_params)
        } else {
            div().child("No builder").into_any_element()
        }
    }
}
```

**Why**: Each outlet independently resolves its child, path changes trigger counter increment (for transitions)

---

## Phase 3: Component Caching (Optional Feature)

### Step 3.1: LRU Cache Implementation

**File**: `src/cache.rs`

```rust
#[cfg(feature = "cache")]
pub struct ComponentCache {
    entries: HashMap<String, CachedEntry>,
    lru_order: VecDeque<String>,
    capacity: usize,
}

#[cfg(feature = "cache")]
struct CachedEntry {
    entity: AnyEntity,
    instance_id: usize,
    last_access: Instant,
}

#[cfg(feature = "cache")]
impl ComponentCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: VecDeque::new(),
            capacity,
        }
    }
    
    pub fn get_or_insert<F>(&mut self, key: &str, create: F) -> AnyEntity
    where
        F: FnOnce() -> AnyEntity
    {
        // Hit: move to back (most recent)
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_access = Instant::now();
            self.lru_order.retain(|k| k != key);
            self.lru_order.push_back(key.to_string());
            return entry.entity.clone();
        }
        
        // Miss: evict if full
        if self.entries.len() >= self.capacity {
            if let Some(oldest_key) = self.lru_order.pop_front() {
                self.entries.remove(&oldest_key);
            }
        }
        
        // Insert new
        let entity = create();
        self.entries.insert(key.to_string(), CachedEntry {
            entity: entity.clone(),
            instance_id: 0, // TODO: use route instance ID
            last_access: Instant::now(),
        });
        self.lru_order.push_back(key.to_string());
        
        entity
    }
    
    pub fn remove(&mut self, key: &str) {
        self.entries.remove(key);
        self.lru_order.retain(|k| k != key);
    }
    
    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
    }
}
```

**Why**: Standard LRU with O(1) operations, bounded memory usage

---

### Step 3.2: Route::component with Caching

**File**: `src/route.rs` (add method)

```rust
impl Route {
    pub fn component<T, F>(path: impl Into<String>, create: F) -> Self
    where
        T: Render + 'static,
        F: Fn() -> T + Send + Sync + 'static + Clone
    {
        let path_str = path.into();
        let key_path = path_str.clone();
        
        Self::new(path_str, move |window, cx, _params| {
            let key = format!("route:{}", key_path);
            let create_fn = create.clone();
            
            let entity = window.use_keyed_state(
                gpui::ElementId::Name(key.into()),
                cx,
                |_, _| create_fn()
            );
            
            entity.clone().into_any_element()
        })
    }
}
```

**Why**: GPUI's `use_keyed_state` automatically caches Entity, integrates with component cache

---

## Phase 4: Testing Strategy

### Unit Tests

**File**: `src/tests/matching.rs`

```rust
#[test]
fn test_segment_matching() {
    let route = Route::new("/user/:id", |_, _, _| div().into_any_element());
    
    let matched = match_path("/user/123", &Arc::new(route));
    assert!(matched.is_some());
    
    let params = matched.unwrap().params;
    assert_eq!(params.get("id"), Some("123"));
}

#[test]
fn test_no_match() {
    let route = Route::new("/user/:id", |_, _, _| div().into_any_element());
    
    let matched = match_path("/product/123", &Arc::new(route));
    assert!(matched.is_none());
}
```

### Integration Tests

**File**: `examples/test_nested.rs`

```rust
fn main() {
    let routes = vec![
        Route::new("/dashboard", dashboard_layout)
            .children(vec![
                Route::new("", overview_page),
                Route::new("analytics", analytics_page),
            ]),
    ];
    
    let mut router = RouterState::new(routes);
    
    // Navigate to /dashboard
    router.navigate("/dashboard".to_string()).unwrap();
    assert_eq!(router.current_path, "/dashboard");
    
    // Resolve child (should get index route)
    let parent = router.current_route.as_ref().unwrap();
    let child = resolve_child_route(parent, "/dashboard", &router.current_params);
    assert!(child.is_some());
    assert_eq!(child.unwrap().route.path, ""); // Index route
}
```

---

## Common Pitfalls & Solutions

### Pitfall 1: Double Borrow in RouterOutlet::render

**Problem**: Calling `router.build(window, cx)` while holding immutable borrow of router

**Solution**: Two-phase rendering - resolve, drop borrow, then build

```rust
// ❌ Bad
let router = cx.global::<RouterState>();
router.current_route.builder(window, cx, params); // Error: can't borrow cx mutably

// ✅ Good
let (route, params) = {
    let router = cx.global::<RouterState>();
    (router.current_route.clone(), router.current_params.clone())
}; // Drop borrow
route.builder(window, cx, &params); // Now cx is free
```

### Pitfall 2: Infinite Render Loop

**Problem**: Outlet triggers navigation which triggers render which triggers navigation...

**Solution**: Path change detection - only update state if path actually changed

```rust
if current_path != self.state.borrow().current_path {
    // Path changed, update state
    self.state.borrow_mut().current_path = current_path;
    // Animation counter increments ONCE per path change
}
```

### Pitfall 3: Lost Component State

**Problem**: Navigating away and back resets component to initial state

**Solution**: Use `Route::component()` which caches via `use_keyed_state`, or enable `cache` feature

---

## Next Steps

1. Implement Phase 1 (core routing) - 1 day
2. Add unit tests for matching logic - 0.5 days
3. Implement Phase 2 (nested routing) - 1 day
4. Add integration tests with examples - 0.5 days
5. Implement Phase 3 (caching) if needed - 0.5 days
6. Performance testing and optimization - 0.5 days

**Total**: ~4 days for full implementation

---

## References

- Research: `research/research.md`
- Data Model: `data-model.md`
- API Contracts: `contracts/api.md`
- Implementation Plan: `plan.md` (next)
