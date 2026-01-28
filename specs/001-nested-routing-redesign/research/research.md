# Nested Routing Architecture Research

**Feature**: 001-nested-routing-redesign  
**Date**: 2026-01-28  
**Research Focus**: Nested routing patterns, infinite loop prevention, stateful component management in Rust UI frameworks

## Research Questions Resolved

1. How do successful routing libraries prevent infinite render loops with nested outlets?
2. How should stateful components be cached and managed in GPUI?
3. What architecture patterns work best for nested route hierarchies?
4. How to integrate with GPUI's Entity lifecycle and reactive state?

---

## Key Findings

### 1. Segment-Based Path Resolution (Industry Standard)

**Decision**: Use segment-based path splitting over regex matching

**Rationale**:
- All surveyed routers (egui_router, React Router, Yew) use segment splitting
- egui_router specifically uses **matchit** crate (axum's router) - efficient trie-based routing
- O(n) lookup where n = number of path segments (vs O(m) for regex with m = pattern complexity)
- Simpler implementation, easier debugging, better performance

**Alternatives Considered**:
- Regex-based matching: More flexible but slower, harder to debug, unnecessary for standard path patterns
- String prefix matching: Simple but doesn't handle parameters or wildcards well

**Implementation**: 
- Path parsing: `"/dashboard/analytics".split('/').filter(|s| !s.is_empty())`
- Parameter extraction: segments starting with `:` are params
- Consider using `matchit` crate for production-grade matching

---

### 2. Stable Component Identity System

**Decision**: Use unique route instance IDs + GPUI's `ElementId` for stable caching

**Rationale**:
- **egui_router pattern**: `AtomicUsize` counter generates unique ID per navigation
- **GPUI pattern**: `window.use_keyed_state(ElementId, ...)` caches components across renders
- Combining both prevents state confusion when same route visited multiple times

**Architecture**:
```rust
// Global counter for route instances
static ROUTE_INSTANCE_ID: AtomicUsize = AtomicUsize::new(0);

// Per-navigation unique ID
struct RouteInstance {
    id: usize,           // Unique forever
    path: String,        // "/dashboard"
    params: RouteParams, // {id: "123"}
}

// GPUI caching
let key = format!("route:{}:{}", route_path, instance_id);
window.use_keyed_state(ElementId::Name(key.into()), cx, || {
    // Component constructor
})
```

**Alternatives Considered**:
- Path-only keys: Fails when same route visited multiple times (state collision)
- Hash-based keys: Works but less debuggable, no clear ordering

---

### 3. Two-Phase Rendering (Resolve → Build)

**Decision**: Separate route resolution phase from component building phase

**Rationale**:
- **Prevents double-borrow**: Router resolution needs immutable borrow, component building needs mutable context
- **egui_router precedent**: Stores `RouteState` with resolved match, then builds component separately
- **React Router pattern**: Route matching occurs first, then outlet renders resolved component

**Implementation Pattern**:
```rust
impl Render for RouterOutlet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // PHASE 1: Resolve (immutable borrow)
        let (child_route, params) = {
            let router = cx.global::<GlobalRouter>();
            let resolved = router.resolve_child(current_path, parent_route);
            (resolved.route.clone(), resolved.params.clone())
        }; // Drop router borrow here
        
        // PHASE 2: Build (mutable context)
        child_route.build(window, cx, &params)
    }
}
```

**Key Benefits**:
- No borrow checker conflicts
- Clear separation of concerns
- Easier testing (can test resolution independently)

---

### 4. Context-Based Nested Architecture

**Decision**: Outlets consume path segments hierarchically, preventing parent re-matching

**Rationale**:
- **Yew nested router insight**: "Child elements don't need awareness of parents"
- **Natural loop prevention**: Once parent consumes `/dashboard`, it can't re-match child's `/analytics`
- **One-way path flow**: Parent → Child → Grandchild (never backwards)

**Example**:
```text
Path: /dashboard/analytics/report
Root RouterOutlet matches:  /dashboard      (consumes 'dashboard')
Dashboard RouterOutlet matches: /analytics  (consumes 'analytics')  
Analytics RouterOutlet matches: /report     (consumes 'report')

Parent CANNOT re-evaluate after child resolves - segments already consumed
```

**This eliminates recursion by design** (not through runtime checks)

---

### 5. Transition State Management

**Decision**: Store both old and new routes during transitions, track animation progress

**Rationale**:
- **egui_router pattern**: `CurrentTransition { active_transition, leaving_route: Option<RouteState> }`
- Old route remains mounted until animation completes
- Progress tracking (`0.0` to `1.0`) determines when to drop old route
- Manual transitions (gestures) use `set_progress()` for external control

**Animation Loop Prevention**:
```rust
if self.progress >= 1.0 && !self.manual_control {
    // Animation complete - drop old route
    return ActiveTransitionResult::Done;
}
// Otherwise continue animating
self.progress += delta_time / duration;
```

**Alternatives Considered**:
- Immediate swap: No transitions, jarring UX
- Crossfade only: Simpler but less flexible than slide/custom transitions

---

### 6. LRU Component Cache Strategy

**Decision**: LRU cache with 10-route capacity (from clarifications), evict on overflow

**Rationale**:
- **React Router precedent**: Keeps recently visited routes in memory
- **Performance**: Instant navigation to cached routes (no re-construction)
- **Memory bound**: LRU eviction prevents unbounded growth
- **GPUI integration**: Use `use_keyed_state` for underlying entity storage

**Implementation Pattern**:
```rust
struct ComponentCache {
    entries: HashMap<String, CachedComponent>, // path -> component
    lru_order: VecDeque<String>,               // Most recent at back
    capacity: usize,                           // Default 10
}

impl ComponentCache {
    fn get_or_insert(&mut self, path: &str, create: impl FnOnce() -> Entity<T>) -> Entity<T> {
        if let Some(entry) = self.entries.get(path) {
            // Move to back (most recent)
            self.lru_order.retain(|p| p != path);
            self.lru_order.push_back(path.to_string());
            return entry.entity.clone();
        }
        
        // Evict if full
        if self.entries.len() >= self.capacity {
            if let Some(oldest) = self.lru_order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        
        // Insert new
        let entity = create();
        self.entries.insert(path.to_string(), CachedComponent { entity: entity.clone() });
        self.lru_order.push_back(path.to_string());
        entity
    }
}
```

**Alternatives Considered**:
- Unlimited cache: Memory leak risk
- No cache: Poor UX, state always resets
- TTL-based eviction: More complex, not necessary for desktop apps

---

### 7. GPUI Dirty Tracking for Loop Prevention

**Decision**: Rely on GPUI's existing dirty tracking + flush cycles (no custom loop prevention)

**Rationale**:
- **GPUI architecture**: `Window::draw()` only renders if `is_dirty()`
- **Flush effects**: `flush_effects()` processes all `cx.notify()` calls before render
- **Cached views**: Automatically skip re-layout if bounds haven't changed
- **Industry gold standard**: Same pattern as Elm, SwiftUI, React Fiber

**How it works**:
```rust
// GPUI's event loop (simplified)
loop {
    handle_events();        // Process user input
    flush_effects();        // Run all cx.notify() updates
    
    if window.is_dirty() {
        window.draw();      // Render once
    }
}
```

**Our navigation flow**:
```rust
Navigator::push(cx, "/new-path");  // Updates GlobalRouter
cx.notify();                       // Marks window dirty
// GPUI ensures single render pass after all effects flush
```

**No custom loop detection needed** - GPUI's architecture prevents it

---

### 8. Navigation Cancellation Strategy

**Decision**: Cancel in-flight navigation when new one starts (from clarifications)

**Rationale**:
- **React Router behavior**: Latest navigation wins
- **Prevents race conditions**: Slow navigation doesn't overwrite fast one
- **User intent**: User's latest action is what they want

**Implementation**:
```rust
struct NavigationState {
    current_path: String,
    pending_navigation: Option<PendingNav>,
    navigation_id: usize, // Increments each navigation
}

fn navigate(new_path: String) {
    // Cancel previous
    if let Some(pending) = self.pending_navigation.take() {
        pending.cancel();
    }
    
    let nav_id = self.navigation_id.fetch_add(1, Ordering::SeqCst);
    self.pending_navigation = Some(PendingNav { id: nav_id, path: new_path });
    
    // Process navigation
    // If nav_id doesn't match current, ignore (was cancelled)
}
```

**Alternatives Considered**:
- Queue all navigations: Shows intermediate states (confusing UX)
- Debounce: Adds artificial delay
- Parallel navigations: Race conditions

---

## Technology Choices

### Core Dependencies

| Dependency | Purpose | Rationale |
|------------|---------|-----------|
| **gpui 0.2.x** | UI framework | Already project dependency, provides Entity/Context system |
| **matchit** (optional) | Path routing | Production-grade trie-based matcher (used by axum), O(n) lookup |
| **lru** | Component cache | Standard LRU cache implementation, small dependency |

### Feature Flags

| Flag | Adds | Justification |
|------|------|---------------|
| `cache` | LRU component caching | Optional for simple apps that don't need state preservation |
| `transition` | Route transition animations | Existing flag, keep for modularity |
| `tracing` | Structured logging | Optional observability (vs `log` flag) |

---

## Architecture Decision Summary

### Chosen: **Context-Based Hierarchical Architecture**

**Pattern**:
```text
GlobalRouter (App-level context)
    ├─ Route Tree (Arc<Route> immutable)
    ├─ RouterState (current path, params)
    └─ ComponentCache (LRU, optional)

RouterOutlet (Component-level)
    ├─ Resolves child route from GlobalRouter
    ├─ Manages child Entity lifecycle
    └─ Handles error boundaries
```

**Why This Works**:
1. **Immutable route tree** (Arc<Route>) - safe to share, clone, access from anywhere
2. **Hierarchical outlets** - each consumes path segments, natural loop prevention
3. **GPUI integration** - uses Entity for caching, Context for updates, dirty tracking for efficiency
4. **Proven pattern** - React Router's `<Outlet>`, Yew's nested routers, egui_router's route states

**Rejected Alternatives**:
- **Centralized state machine**: Too rigid, doesn't compose well
- **Imperative navigation stack**: Doesn't fit GPUI's reactive model
- **Redux-style actions**: Overkill for routing, adds boilerplate

---

## Implementation Risks & Mitigations

### Risk 1: Double-Borrow in RouterOutlet::render

**Mitigation**: Two-phase rendering (resolve → drop borrow → build)

### Risk 2: State Loss on Cache Eviction

**Mitigation**: LRU warnings in docs + explicit cleanup API for developers

### Risk 3: Infinite Loops from Reactive Updates

**Mitigation**: GPUI's dirty tracking + path change detection (only notify if path actually changed)

### Risk 4: Performance with Deep Nesting

**Mitigation**: Segment-based matching is O(depth), acceptable for typical apps (<5 levels)

---

## References

1. **egui_router**: https://github.com/lucasmerlin/hello_egui/tree/main/crates/egui_router
   - Studied: Animation system, route state management, ID generation
   
2. **Yew nested router**: https://github.com/ctron/yew-nested-router
   - Studied: Context-based architecture, segment consumption
   
3. **React Router**: https://reactrouter.com/en/main/start/concepts#nested-routes
   - Studied: Outlet pattern, route hierarchy, navigation guards
   
4. **GPUI Framework**: https://github.com/zed-industries/zed
   - Studied: Entity lifecycle, dirty tracking, use_keyed_state
   
5. **matchit crate**: https://docs.rs/matchit/latest/matchit/
   - Potential dependency for production routing

---

## Next Steps

1. ✅ Phase 0 Complete - Research findings documented
2. → Phase 1: Design data model and API contracts
3. → Phase 2: Write implementation tasks
