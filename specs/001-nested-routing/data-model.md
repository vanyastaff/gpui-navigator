# Data Model: Nested Routing

**Feature**: 001-nested-routing  
**Date**: 2026-01-28  
**Purpose**: Define data structures and their relationships for nested routing improvements

## Core Entities

### Route

**Purpose**: Immutable routing configuration defining a navigation destination

**Fields**:
- `path: String` - Route path pattern (e.g., "/dashboard", "/:id", "settings")
- `builder: RouteBuilder` - Function that constructs the UI element for this route
- `children: Vec<Arc<Route>>` - Default child routes (rendered in unnamed RouterOutlet)
- `named_children: HashMap<String, Vec<Arc<Route>>>` - Named child routes (rendered in RouterOutlet::named())
- `name: Option<String>` - Optional route name for named navigation
- `transition: Option<Transition>` - Animation when navigating to/from this route
- `guard: Option<RouteGuard>` - Optional authentication/authorization check (feature flag)
- `middleware: Vec<RouteMiddleware>` - Optional before/after hooks (feature flag)

**Relationships**:
- **Has-Many**: Children (recursive self-reference)
- **Has-Many**: Named children (grouped by outlet name)

**Invariants**:
- Path MUST NOT be empty for non-root routes
- Child paths MUST be relative (no leading slash) - ENFORCED in validation
- At most ONE index route (empty path "") per children vec - ENFORCED in validation
- Named children keys MUST NOT be empty strings

**Normalization**:
- Paths are stored AS-IS (with slashes) for transparency
- Normalized on-demand during resolution using `normalize_path()`
- Root route "/" is special-cased

---

### RouteParams

**Purpose**: Map of route parameters extracted from URL segments

**Structure**:
```rust
pub struct RouteParams {
    params: HashMap<String, String>
}
```

**Fields** (internal):
- `params: HashMap<String, String>` - Parameter name → value mapping

**Operations**:
- `new() -> Self` - Create empty parameter map
- `get(&self, key: &str) -> Option<&String>` - Retrieve parameter value
- `insert(&mut self, key: String, value: String)` - Add parameter
- `merge(&mut self, other: &RouteParams)` - Merge params (child params override parent)
- `clone(&self) -> Self` - Clone for passing to children

**Invariants**:
- Parameter names MUST NOT contain `{}` (constraints are stripped before insertion)
- Parameter values MUST NOT be empty strings
- Parameter names are case-sensitive

**Examples**:
```rust
// Single parameter
{id: "123"}

// Multiple parameters (same level)
{userId: "123", tab: "profile"}

// Inherited parameters (parent + child)
{workspaceId: "abc", projectId: "456"}
```

---

### ResolvedChildRoute

**Purpose**: Result of successful child route resolution

**Structure**:
```rust
pub type ResolvedChildRoute = (Arc<Route>, RouteParams);
```

**Components**:
- `Arc<Route>` - The matched child route (Arc for cheap cloning)
- `RouteParams` - Merged parameters from parent + child

**Lifetime**:
- Created by `resolve_child_route()`
- Consumed by `RouterOutlet` to render matched child
- Short-lived (per render cycle unless cached)

**Usage**:
```rust
if let Some((child_route, params)) = resolve_child_route(...) {
    // child_route: Arc<Route> - use for rendering
    // params: RouteParams - pass to child component
}
```

---

### PathSegment (Internal)

**Purpose**: Individual component of a route path after splitting

**Structure**: `&str` (borrowed slice from parent string)

**Types**:
1. **Static segment**: Literal string (e.g., "dashboard", "settings")
2. **Parameter segment**: Starts with `:` (e.g., ":id", ":userId{uuid}")
3. **Empty segment**: "" (from multiple consecutive slashes or root path)

**Matching Rules**:
- Static segments match via equality (`segment == pattern`)
- Parameter segments match any non-empty string (`pattern.starts_with(':')`)
- Empty segments are filtered out during splitting

**Example**:
```
Path: "/dashboard/users/123/profile"
Segments: ["dashboard", "users", "123", "profile"]

Pattern: "/dashboard/:userId/profile"
Pattern Segments: ["dashboard", ":userId", "profile"]

Matching:
  "dashboard" == "dashboard" ✓
  "users" != ":userId" but :userId is parameter → match "users" ✓
  "123" == "profile"? No, but we already matched... (BUG - see research.md BUG-002)
```

---

### NormalizedPath (Internal Helper)

**Purpose**: Canonical representation of a route path

**Structure**: `Cow<'a, str>` (borrowed when already normalized, owned when modified)

**Normalization Rules**:
1. Remove all leading slashes: `"/dashboard"` → `"dashboard"`
2. Remove all trailing slashes: `"dashboard/"` → `"dashboard"`
3. Root path "/" becomes empty string `""`
4. Multiple consecutive slashes preserved internally, filtered during splitting

**Implementation**:
```rust
pub(crate) fn normalize_path(path: &str) -> Cow<'_, str> {
    let trimmed = path.trim_matches('/');
    if trimmed == path {
        Cow::Borrowed(path)  // Already normalized, avoid allocation
    } else {
        Cow::Owned(trimmed.to_string())  // Modified, return owned
    }
}
```

**Usage**:
```rust
let normalized = normalize_path("/dashboard/");
assert_eq!(normalized, "dashboard");
```

---

### ResolutionKey (Cache Key)

**Purpose**: Unique key for caching route resolution results

**Structure**:
```rust
#[cfg(feature = "cache")]
#[derive(Hash, Eq, PartialEq, Clone)]
struct ResolutionKey {
    parent_path: String,
    current_path: String,
    outlet_name: Option<String>,
}
```

**Fields**:
- `parent_path: String` - Parent route's path
- `current_path: String` - Current navigation path
- `outlet_name: Option<String>` - Named outlet (None for default)

**Hashing**:
- All three fields must match for cache hit
- Case-sensitive comparison
- Slashes are NOT normalized in key (normalization happens before lookup)

**Cache Strategy**:
- LRU cache with max 100 entries (configurable via feature flag)
- Thread-local storage (no cross-thread contention)
- Cleared on navigation context reset

---

## Relationships Diagram

```
┌─────────────┐
│    Route    │
│             │
│  - path     │───┐
│  - builder  │   │ children (Vec<Arc<Route>>)
│  - children │◄──┘
│  - named_children (HashMap)
│  - name     │
│  - transition
└─────────────┘
      │
      │ rendered by
      ▼
┌─────────────────┐
│  RouterOutlet   │
│                 │
│  - name: Option │
│  - current_route│────────┐
└─────────────────┘        │
                           │ resolves to
                           ▼
              ┌───────────────────────┐
              │ ResolvedChildRoute    │
              │                       │
              │  (Arc<Route>,         │
              │   RouteParams)        │
              └───────────────────────┘
                           │
                           │ contains
                           ▼
                   ┌────────────────┐
                   │  RouteParams   │
                   │                │
                   │  {key: value}  │
                   └────────────────┘
```

---

## State Transitions

### Route Resolution State Machine

```
┌─────────────────┐
│  START          │
│  (current_path) │
└────────┬────────┘
         │
         ▼
┌─────────────────────┐
│ Normalize Paths     │
│ (parent + current)  │
└────────┬────────────┘
         │
         ▼
┌──────────────────────┐
│ Extract Remaining    │
│ (current - parent)   │
└────────┬─────────────┘
         │
         ├──► remaining.is_empty()? ──► find_index_route()
         │                                   │
         │                                   ├─► Found ──► RETURN (index, params)
         │                                   └─► None ──► RETURN None
         │
         ▼
┌──────────────────────┐
│ Split into Segments  │
│ ["seg1", "seg2",...]│
└────────┬─────────────┘
         │
         ▼
┌──────────────────────┐
│ Match First Segment  │
│ Against Children     │
└────────┬─────────────┘
         │
         ├──► Static Match? ──► Extract Params (if param route)
         │                           │
         │                           ▼
         │                      ┌────────────────────┐
         │                      │ Remaining Segments?│
         │                      └─────┬──────────────┘
         │                            │
         │                            ├─► YES ──► Recurse into Child
         │                            │             └──► RETURN (child, merged_params)
         │                            │
         │                            └─► NO ──► RETURN (child, params)
         │
         └──► No Match ──► RETURN None
```

---

## Performance Characteristics

### Space Complexity

- **Route**: O(1) per route + O(c) for children + O(n) for named_children
  - `c` = number of children
  - `n` = total routes in named outlets
  - Arc sharing reduces memory overhead

- **RouteParams**: O(p) where p = number of parameters
  - Typically p ≤ 5 for nested routes

- **Resolution Cache**: O(k) where k = cache size (default 100 entries)
  - Each entry: ~200 bytes (key + Arc<Route> + params)
  - Total: ~20 KB

### Time Complexity

- **resolve_child_route** (uncached):
  - Path normalization: O(|path|)
  - Segment splitting: O(|segments|)
  - Child matching: O(c) where c = number of children
  - **Total**: O(|path| + |segments| + c) ≈ O(n) where n = path length

- **resolve_child_route** (cached):
  - Cache lookup: O(1) (hash map)
  - **Total**: O(1)

- **Recursive resolution** (deep nesting):
  - Each level: O(|path| + c)
  - d levels: O(d * (|path| + c))
  - **Worst case**: O(depth * children_per_level)

**Target**: <1ms for typical cases (depth ≤ 5, children ≤ 20)

---

## Validation Rules

### Route Validation (Enforced in `Route::children()`)

1. **Child Paths Must Be Relative**:
   ```rust
   // INVALID:
   Route::new("/dashboard", ...).children(vec![
       Route::new("/absolute", ...)  // ❌ Leading slash
   ])

   // VALID:
   Route::new("/dashboard", ...).children(vec![
       Route::new("relative", ...)  // ✓ No leading slash
   ])
   ```

2. **Single Index Route**:
   ```rust
   // INVALID:
   Route::new("/parent", ...).children(vec![
       Route::new("", ...),      // ❌ Two index routes
       Route::new("index", ...)  // ❌
   ])

   // VALID:
   Route::new("/parent", ...).children(vec![
       Route::new("", ...),         // ✓ One index
       Route::new("settings", ...)  // ✓ Non-index
   ])
   ```

3. **Named Outlet Keys Not Empty**:
   ```rust
   // INVALID:
   parent_route.named_children.insert("".to_string(), vec![...]);  // ❌

   // VALID:
   parent_route.named_children.insert("sidebar".to_string(), vec![...]);  // ✓
   ```

### Parameter Validation

1. **Parameter Names Without Constraints**:
   ```rust
   // Input: ":id{uuid}"
   // Stored: "id" (constraint {uuid} stripped)
   ```

2. **Parameter Values Not Empty**:
   ```rust
   // INVALID:
   params.insert("id".to_string(), "".to_string());  // ❌ Empty value

   // VALID:
   params.insert("id".to_string(), "123".to_string());  // ✓
   ```

---

## Example Scenarios

### Scenario 1: Simple Nested Route

**Route Tree**:
```rust
Route::new("/dashboard", DashboardLayout)
    .children(vec![
        Route::new("", OverviewPage),      // Index
        Route::new("settings", SettingsPage),
    ])
```

**Navigation to `/dashboard`**:
1. Match parent "/dashboard"
2. Remaining path: "" (empty)
3. Call `find_index_route()` → finds child with path ""
4. Returns `(OverviewPage route, {})`

**Navigation to `/dashboard/settings`**:
1. Match parent "/dashboard"
2. Remaining path: "settings"
3. Split segments: ["settings"]
4. Match "settings" against children → exact match
5. Returns `(SettingsPage route, {})`

---

### Scenario 2: Parameter Inheritance

**Route Tree**:
```rust
Route::new("/workspace/:wid", WorkspaceLayout)
    .children(vec![
        Route::new("projects/:pid", ProjectPage),
    ])
```

**Navigation to `/workspace/abc/projects/123`**:
1. Match parent "/workspace/:wid"
2. Extract parent param: {wid: "abc"}
3. Remaining path: "projects/123"
4. Resolve child "projects/:pid"
5. Extract child param: {pid: "123"}
6. Merge: {wid: "abc", pid: "123"}
7. Returns `(ProjectPage route, {wid: "abc", pid: "123"})`

---

### Scenario 3: Named Outlets

**Route Tree**:
```rust
Route::new("/app", AppLayout)
    .children(vec![
        Route::new("content", ContentPage),
    ])
    .named_children("sidebar", vec![
        Route::new("nav", SidebarNav),
    ])
```

**Rendering**:
```rust
// Main content outlet (default)
RouterOutlet::new()  → resolves to "content" child

// Sidebar outlet (named)
RouterOutlet::named("sidebar")  → resolves to "nav" child
```

---

## Migration Notes

### Breaking Changes

None. All changes are internal optimizations and bug fixes.

### Deprecations

- **Absolute child paths**: Will log warning in 0.1.4, reject in 0.2.0
- **Multiple index routes**: Will log warning in 0.1.4, reject in 0.2.0
- **`println!` debug output**: Replaced with `trace_log!` in 0.1.4

### New Features

- **Parameter constraint stripping**: Automatic in 0.1.4
- **Recursive parameter extraction**: Automatic in 0.1.4
- **Resolution caching**: Opt-in via `cache` feature flag

---

## Testing Data

### Test Fixtures

```rust
// Minimal route tree
fn minimal_tree() -> Route {
    Route::new("/", HomePage)
        .children(vec![
            Route::new("about", AboutPage),
        ])
}

// Deep nesting (3 levels)
fn deep_tree() -> Route {
    Route::new("/", Root)
        .children(vec![
            Route::new("level1", Level1)
                .children(vec![
                    Route::new("level2", Level2)
                        .children(vec![
                            Route::new("level3", Level3),
                        ]),
                ]),
        ])
}

// Parameter inheritance
fn param_tree() -> Route {
    Route::new("/users/:userId", UserLayout)
        .children(vec![
            Route::new("posts/:postId", PostPage),
        ])
}

// Named outlets
fn named_outlet_tree() -> Route {
    Route::new("/app", AppLayout)
        .children(vec![
            Route::new("main", MainContent),
        ])
        .named_children("sidebar", vec![
            Route::new("nav", SidebarNav),
        ])
}
```

### Test Cases

| Test ID | Input Path | Route Tree | Expected Result | Test Type |
|---------|-----------|------------|-----------------|-----------|
| T001 | "/about" | minimal_tree() | (AboutPage, {}) | Unit |
| T002 | "/level1/level2/level3" | deep_tree() | (Level3, {}) | Integration |
| T003 | "/users/123/posts/456" | param_tree() | (PostPage, {userId: "123", postId: "456"}) | Integration |
| T004 | "/" | minimal_tree() | find index or None | Unit |
| T005 | "/app/main" (default outlet) | named_outlet_tree() | (MainContent, {}) | Unit |
| T006 | "/app/nav" (named outlet "sidebar") | named_outlet_tree() | (SidebarNav, {}) | Unit |
| T007 | "/dashboard/" (trailing slash) | dashboard_tree() | Same as "/dashboard" | Edge Case |
| T008 | "//dashboard" (leading slashes) | dashboard_tree() | Same as "/dashboard" | Edge Case |

---

## Conclusion

The data model refactoring focuses on:
1. **Immutability**: Routes are Arc-wrapped, parameters are cloned when merged
2. **Efficient borrowing**: Cow<str> for paths, borrowed segments during matching
3. **Clear ownership**: Arc<Route> for cheap cloning, HashMap for O(1) parameter lookup
4. **Validation**: Enforce constraints at Route creation time, not resolution time

All entities support the constitution's requirements:
- **Type Safety** (Principle V): Arc, Cow, HashMap with clear ownership
- **Performance** (Principle V): <1ms resolution target via caching + optimizations
- **Rust Idioms** (Principle V): No .clone() except where necessary, prefer borrowing
