# Data Model: Nested Routing Architecture

**Feature**: 001-nested-routing-redesign  
**Date**: 2026-01-28

## Core Entities

### 1. Route

**Purpose**: Represents a single route definition in the hierarchy

**Fields**:
```rust
pub struct Route {
    /// Route path segment (e.g., "dashboard", ":id", "")
    /// Empty string "" indicates index route
    path: String,
    
    /// Optional route builder function
    /// Fn(window, context, params) -> AnyElement
    builder: Option<RouteBuilder>,
    
    /// Child routes (nested hierarchy)
    children: Vec<Arc<Route>>,
    
    /// Optional route name for named navigation
    name: Option<String>,
    
    /// Transition configuration (optional feature)
    #[cfg(feature = "transition")]
    transition: TransitionConfig,
    
    /// Route guards (optional feature)
    #[cfg(feature = "guard")]
    guards: Vec<BoxedGuard>,
}
```

**Relationships**:
- Parent-child via `children: Vec<Arc<Route>>`
- Immutable via `Arc` wrapping for thread-safe sharing

**Validation Rules**:
- Path segments must not start with `/` (relative to parent)
- Index routes have `path = ""`
- Parameter paths start with `:` (e.g., `:id`)
- Circular dependencies detected during route tree construction

**State Transitions**: N/A (immutable after construction)

---

### 2. RouterState

**Purpose**: Maintains current navigation state

**Fields**:
```rust
pub struct RouterState {
    /// Current active path
    current_path: String,
    
    /// Current route params (merged from hierarchy)
    current_params: RouteParams,
    
    /// Currently matched route (deepest in hierarchy)
    current_route: Option<Arc<Route>>,
    
    /// Root routes (top-level)
    routes: Vec<Arc<Route>>,
    
    /// Navigation counter (increments per navigation)
    navigation_id: AtomicUsize,
    
    /// Component cache (optional feature)
    #[cfg(feature = "cache")]
    component_cache: ComponentCache,
}
```

**Relationships**:
- Contains root `routes` tree
- References `current_route` within tree

**Validation Rules**:
- `current_path` must start with `/`
- `current_route` must exist in `routes` tree if Some
- `navigation_id` strictly increasing

**State Transitions**:
```text
Idle → Navigating (path change requested)
Navigating → Resolved (route match found)
Resolved → Rendered (component built)
Rendered → Idle (navigation complete)
```

---

### 3. RouteParams

**Purpose**: Key-value map of extracted path parameters

**Fields**:
```rust
pub struct RouteParams {
    /// Parameter map (e.g., {"id": "123", "category": "books"})
    params: HashMap<String, String>,
}
```

**Relationships**:
- Owned by RouterState
- Passed to route builders
- Merged hierarchically (parent params + child params)

**Validation Rules**:
- Keys must be valid identifiers (alphanumeric + underscore)
- Child params override parent params on collision

---

### 4. RouterOutlet

**Purpose**: Placeholder component that renders child routes

**Fields**:
```rust
pub struct RouterOutlet {
    /// Optional outlet name (default is None)
    name: Option<String>,
    
    /// Internal state (current path, animation counter)
    state: RefCell<OutletState>,
}
```

**Sub-entity**: `OutletState`
```rust
struct OutletState {
    /// Last rendered path
    current_path: String,
    
    /// Animation counter (increments on path change)
    animation_counter: u32,
    
    /// Current route params
    current_params: RouteParams,
    
    /// Current route builder
    current_builder: Option<RouteBuilder>,
    
    /// Previous route (for exit animations)
    #[cfg(feature = "transition")]
    previous_route: Option<PreviousRoute>,
    
    /// Active transition
    #[cfg(feature = "transition")]
    current_transition: Transition,
}
```

**Relationships**:
- Lives within parent route's component
- Queries GlobalRouter for child routes
- Manages child component lifecycle

**Validation Rules**:
- Named outlets must have unique names within parent
- Default outlet has `name = None`

**State Transitions**:
```text
Empty → HasChild (child route resolved)
HasChild → Transitioning (path changed, animation starting)
Transitioning → HasChild (animation complete)
HasChild → Empty (no child route found)
```

---

### 5. ComponentCache (Optional)

**Purpose**: LRU cache for stateful route components

**Fields**:
```rust
#[cfg(feature = "cache")]
struct ComponentCache {
    /// Map of path → cached component entity
    entries: HashMap<String, CachedEntry>,
    
    /// LRU order (most recent at back)
    lru_order: VecDeque<String>,
    
    /// Cache capacity (default 10)
    capacity: usize,
}

struct CachedEntry {
    /// Component entity handle
    entity: AnyEntity,
    
    /// Route instance ID (for uniqueness)
    instance_id: usize,
    
    /// Last accessed timestamp
    last_access: Instant,
}
```

**Relationships**:
- Owned by RouterState
- Stores GPUI Entity handles

**Validation Rules**:
- `capacity` must be > 0
- `entries.len()` ≤ `capacity`
- `lru_order.len()` == `entries.len()`

**Cache Eviction**:
```text
When entries.len() == capacity AND new insert:
    1. Pop oldest from lru_order.front()
    2. Remove from entries
    3. Drop Entity (GPUI cleans up)
    4. Insert new entry
    5. Push to lru_order.back()
```

---

### 6. Navigator (API)

**Purpose**: Public API for programmatic navigation

**Methods** (not data, but defines interaction):
```rust
impl Navigator {
    /// Push new path (adds to history)
    pub fn push(cx: &mut Context, path: String);
    
    /// Replace current path (no history entry)
    pub fn replace(cx: &mut Context, path: String);
    
    /// Navigate back (if history exists)
    pub fn back(cx: &mut Context);
    
    /// Navigate forward (if history exists)
    pub fn forward(cx: &mut Context);
    
    /// Clear component cache for path
    #[cfg(feature = "cache")]
    pub fn clear_cache(cx: &mut Context, path: &str);
    
    /// Clear entire component cache
    #[cfg(feature = "cache")]
    pub fn clear_all_cache(cx: &mut Context);
}
```

**State Changes**:
- All methods update `GlobalRouter` in GPUI's App context
- All methods call `cx.notify()` to trigger re-render

---

## Data Flow Diagrams

### Navigation Flow

```text
User Action (click link)
    ↓
Navigator::push("/dashboard/analytics")
    ↓
GlobalRouter.update_state(new_path)
    ↓
cx.notify() → Marks window dirty
    ↓
GPUI flush_effects()
    ↓
RouterOutlet::render()
    ↓
    [Phase 1: Resolve]
    GlobalRouter.resolve_child(current_path, parent_route)
        → Returns (child_route, params)
    ↓
    [Phase 2: Build]
    child_route.build(window, cx, params)
        → Returns AnyElement
    ↓
GPUI Window::draw()
    → Renders component tree
```

### Route Resolution Flow (Hierarchical)

```text
Path: "/dashboard/analytics/report"

Root RouterOutlet:
    Consumes "dashboard"
    Matches Route { path: "dashboard", children: [...] }
    Renders DashboardLayout
        ↓
Dashboard RouterOutlet (within DashboardLayout):
    Consumes "analytics"
    Matches Route { path: "analytics", children: [...] }
    Renders AnalyticsLayout
        ↓
Analytics RouterOutlet (within AnalyticsLayout):
    Consumes "report"
    Matches Route { path: "report", children: [] }
    Renders ReportPage (leaf)

Parent NEVER re-evaluates after child matches → No recursion
```

### Component Caching Flow

```text
[With cache feature enabled]

Navigation to /counter:
    ↓
ComponentCache.get("/counter")
    → MISS (first visit)
    ↓
Build new CounterPage entity
    ↓
ComponentCache.insert("/counter", entity, instance_id=1)
    ↓
Render CounterPage (state: count=0)

User increments counter → count=5

Navigation to /home:
    ↓
Counter entity remains in cache
    → LRU order: ["/counter"]

Navigation back to /counter:
    ↓
ComponentCache.get("/counter")
    → HIT (cache entry found)
    ↓
Return cached entity (no re-construction)
    ↓
Render CounterPage (state: count=5) ← State preserved!

If 10 more routes visited after /counter:
    ↓
ComponentCache exceeds capacity
    ↓
Evict "/counter" (oldest inactive)
    ↓
Drop CounterPage entity → State lost

Next visit to /counter:
    ↓
MISS → Reconstruct → count=0 again
```

---

## Invariants

### Global Invariants

1. **Route Tree Immutability**: `Arc<Route>` tree never mutates after construction
2. **Unique Navigation IDs**: `navigation_id` strictly increasing (monotonic)
3. **Path Normalization**: All paths stored with leading `/`, no trailing `/`
4. **Single Active Navigation**: Only one navigation in-flight at a time (cancellation-based)

### Outlet Invariants

1. **Path Change Detection**: Outlet only updates state if `current_path != new_path`
2. **Animation Counter Increment**: Counter only increments on actual path changes
3. **No Orphan Outlets**: Outlet without matching parent route renders nothing gracefully

### Cache Invariants (if enabled)

1. **Capacity Bound**: `cache.entries.len() ≤ cache.capacity` always
2. **LRU Consistency**: `cache.lru_order.len() == cache.entries.len()` always
3. **Instance Uniqueness**: Same path visited twice has different `instance_id`

---

## Entity Lifecycle Management

### Route Component Lifecycle

```text
Construction:
    Route::component("/counter", CounterPage::new)
    ↓
Registration:
    GlobalRouter.add_route(route)
    → Route stored in Arc, shared across app
    ↓
First Navigation:
    window.use_keyed_state(key, cx, || CounterPage::new())
    → Entity created, stored in GPUI EntityMap
    ↓
Subsequent Navigations (within cache):
    window.use_keyed_state(key, cx, || ...)
    → Returns existing Entity (no reconstruction)
    ↓
Cache Eviction OR Explicit Cleanup:
    cache.remove(path)
    → Entity handle dropped
    → GPUI EntityMap removes entity on next GC
    ↓
Destruction:
    Entity dropped from EntityMap
    → Component state destroyed
```

### Outlet Lifecycle

```text
Parent Route Rendered:
    parent.build() → Returns AnyElement with RouterOutlet
    ↓
Outlet Constructed:
    RouterOutlet::new() or RouterOutlet::named("sidebar")
    ↓
Outlet Rendered:
    Outlet::render() called every frame parent is visible
    → Resolves child route
    → Builds child component
    ↓
Path Changes:
    Outlet detects path != current_path
    → Updates internal state
    → Increments animation_counter (if transitions enabled)
    → Triggers transition
    ↓
Parent Route Unmounted:
    Outlet dropped
    → Internal state dropped
    → Child component remains in cache (if enabled)
```

---

## Error States

### Route Resolution Errors

| Error | Condition | Handling |
|-------|-----------|----------|
| **NoMatchFound** | Path doesn't match any route | Render not-found page |
| **CircularDependency** | Route tree has cycle | Panic during construction (dev-time) |
| **InvalidPath** | Path malformed (e.g., "//foo") | Normalize or reject |
| **MissingParameter** | Required param not in path | Render error UI in outlet |

### Component Errors

| Error | Condition | Handling |
|-------|-----------|----------|
| **BuilderPanic** | Route builder panics | Catch, render error UI in outlet, parent layout remains |
| **RenderPanic** | Component render panics | GPUI error boundary catches, show error |
| **DoubleB orrow** | Borrow conflict during build | Two-phase rendering prevents this |

### Cache Errors

| Error | Condition | Handling |
|-------|-----------|----------|
| **CacheFull** | Cache at capacity | Evict LRU entry automatically |
| **InvalidKey** | Key format wrong | Sanitize or reject |
| **EntityDropped** | Cached entity no longer valid | Remove from cache, rebuild |

---

## Performance Characteristics

### Time Complexity

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Route Resolution | O(depth × siblings) | Depth typically <5, siblings <10 |
| Parameter Extraction | O(segments) | Segments = path depth |
| Cache Lookup | O(1) | HashMap get |
| Cache Eviction | O(1) | VecDeque pop_front |
| Component Build | O(1) | Direct function call |

### Space Complexity

| Structure | Size | Notes |
|-----------|------|-------|
| Route Tree | O(routes) | Shared via Arc, minimal overhead |
| RouterState | O(1) | Single current state |
| ComponentCache | O(capacity) | Bounded (default 10) |
| RouteParams | O(params) | Typically 1-3 params |

### Memory Bounds

With cache enabled (capacity=10):
- **Max cached components**: 10 entities
- **Entity size**: ~few KB per component (depends on state)
- **Total cache overhead**: <100KB typical

Without cache:
- **Memory usage**: O(routes) for route tree only
- **Component lifecycle**: Destroyed immediately on unmount

---

## Validation & Constraints Summary

### Construction-Time Validations

- ✅ No circular dependencies in route tree
- ✅ Parameter names are valid identifiers
- ✅ Paths are relative (no leading `/` in children)
- ✅ Index routes have `path = ""`

### Runtime Validations

- ✅ Current path exists in route tree (or not-found)
- ✅ Component builder succeeds (or error boundary)
- ✅ Cache capacity never exceeded
- ✅ Navigation IDs monotonically increasing

### Developer Contracts

- ⚠️ Route builders must not panic (or UI breaks)
- ⚠️ Component state should be reasonable size (<1MB)
- ⚠️ Don't create thousands of routes (impacts resolution time)
- ⚠️ Cache capacity should match app navigation patterns

---

## Next Steps

- [x] Data model defined
- [ ] API contracts (public interfaces)
- [ ] Quickstart guide
- [ ] Implementation plan
