//! Route resolution via Match Stack
//!
//! # Architecture
//!
//! Instead of each `RouterOutlet` independently searching the route tree at render time,
//! we resolve the **entire chain of matched routes once** when navigation occurs.
//!
//! The result is a `MatchStack` — an ordered list of `MatchEntry` items, one per nesting level.
//! Each `RouterOutlet` simply reads its entry by depth index.
//!
//! # Example
//!
//! Given routes:
//! ```text
//! /              (root layout)
//!   dashboard    (has children)
//!     ""         (index → overview)
//!     settings   (has children)
//!       profile  (leaf)
//!     :id        (param leaf)
//! ```
//!
//! For path `/dashboard/settings/profile`, the match stack is:
//! ```text
//! [0] Route("/")          params={}                 ← router_view renders this
//! [1] Route("dashboard")  params={}                 ← outlet depth 1
//! [2] Route("settings")   params={}                 ← outlet depth 2
//! [3] Route("profile")    params={}                 ← outlet depth 3
//! ```
//!
//! For path `/dashboard/42`:
//! ```text
//! [0] Route("/")          params={}                 ← router_view renders this
//! [1] Route("dashboard")  params={}                 ← outlet depth 1
//! [2] Route(":id")        params={id: "42"}         ← outlet depth 2
//! ```
//!
//! # Depth Tracking
//!
//! Outlets discover their depth via a thread-local counter:
//! - `router_view()` resets depth to 0, renders `match_stack[0]`
//! - Each outlet sets depth = parent_depth + 1 and renders `match_stack[depth]`
//! - Works for both functional (`render_router_outlet`) and entity (`RouterOutlet`) APIs

use crate::nested::normalize_path;
use crate::route::Route;
use crate::{debug_log, trace_log, warn_log, RouteParams};
use std::cell::Cell;
use std::sync::Arc;

// ============================================================================
// Depth Tracking (thread-local, PARENT_DEPTH approach)
// ============================================================================
//
// # Why not NESTING counter?
//
// GPUI renders Entity<T> **after** the parent builder returns — NOT inline.
// `Route::component()` returns `Entity<T>.into_any_element()` as a blueprint;
// GPUI calls `T::render()` later during layout/paint.
//
// This means any save/restore pattern (enter/exit with NESTING counter)
// breaks: by the time child `RouterOutlet::render()` runs, the parent
// already called `exit_outlet()` and the counter is reset.
//
// # Solution: PARENT_DEPTH
//
// A single thread-local `Option<usize>`:
// - `None` → next outlet is ROOT → depth = 0
// - `Some(d)` → next outlet is CHILD of depth `d` → depth = d + 1
//
// Each outlet:
// 1. Reads PARENT_DEPTH to determine its own depth
// 2. Sets PARENT_DEPTH = Some(my_depth) before `route.build()`
// 3. Does NOT restore PARENT_DEPTH after build
//
// This works because GPUI renders depth-first: when child `T::render()` runs,
// PARENT_DEPTH still holds the value set by its parent outlet.
//
// # Render flow
//
// ```text
// NestedDemoApp::render()                   PARENT_DEPTH=None
//   └─ .child(self.outlet.clone())
//      // GPUI calls RouterOutlet::render()
//      RouterOutlet::render()
//        PARENT_DEPTH=None → ROOT → my_depth=0
//        set PARENT_DEPTH=Some(0)
//        route.build() → Entity<DashboardLayout>.into_any_element()
//        (no restore!)
//
//      // GPUI processes element tree, calls DashboardLayout::render()
//      DashboardLayout::render()            PARENT_DEPTH=Some(0)
//        .child(outlet.clone())
//        // GPUI calls child RouterOutlet::render()
//        RouterOutlet::render()
//          PARENT_DEPTH=Some(0) → CHILD → my_depth=1
//          set PARENT_DEPTH=Some(1)
//          route.build() → AnalyticsPage
//          stack.at_depth(1) → Route("analytics")
// ```

thread_local! {
    /// Depth of the parent outlet that last called `route.build()`.
    /// `None` means no parent → next outlet is root (depth 0).
    /// `Some(d)` means parent is at depth `d` → next outlet is at `d + 1`.
    ///
    /// Used ONLY for initial depth discovery when an outlet first renders.
    /// After that, outlets store their depth in their own field.
    static PARENT_DEPTH: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Discover the depth for a NEW outlet rendering for the first time.
///
/// Returns `my_depth` — the match stack index this outlet should render.
///
/// - If `PARENT_DEPTH` is `None`: this is ROOT → depth = 0
/// - If `PARENT_DEPTH` is `Some(d)`: this is CHILD → depth = d + 1
///
/// Also sets `PARENT_DEPTH = Some(my_depth)` so that child outlets
/// created inside this outlet's builder get the correct depth.
///
/// This should only be called ONCE per outlet (on first render).
/// After that, use `set_parent_depth()` with the saved depth.
pub fn enter_outlet() -> usize {
    let parent = PARENT_DEPTH.with(Cell::get);

    let my_depth = match parent {
        None => 0,        // ROOT outlet
        Some(d) => d + 1, // CHILD outlet
    };

    // Set for children rendered inside our builder
    PARENT_DEPTH.with(|p| p.set(Some(my_depth)));

    my_depth
}

/// Set PARENT_DEPTH to `depth` so child outlets see the correct parent.
///
/// Called by outlets that already know their depth (from a previous render).
/// This ensures that child outlets created via `enter_outlet()` or
/// rendered as deferred Entity components get `depth + 1`.
pub fn set_parent_depth(depth: usize) {
    PARENT_DEPTH.with(|p| p.set(Some(depth)));
}

/// Reset outlet tracking state to "no parent".
///
/// Called by `router_view()` at the start of a render cycle,
/// or between render passes to ensure clean state.
pub fn reset_outlet_depth() {
    PARENT_DEPTH.with(|p| p.set(None));
}

/// Get current outlet depth without modifying state. Used by named outlets.
pub fn current_outlet_depth() -> usize {
    PARENT_DEPTH.with(|p| p.get().map_or(0, |d| d + 1))
}

/// Get the raw parent depth value (for debugging/testing).
pub fn current_parent_depth() -> Option<usize> {
    PARENT_DEPTH.with(Cell::get)
}

// ============================================================================
// Match Stack
// ============================================================================

/// A single entry in the route match stack.
///
/// Represents one level of the route hierarchy that matched the current path.
#[derive(Debug, Clone)]
pub struct MatchEntry {
    /// The matched route at this level
    pub route: Arc<Route>,
    /// Accumulated params (includes all params from parent levels + this level)
    pub params: RouteParams,
    /// Depth in the hierarchy (0 = root/top-level route)
    pub depth: usize,
}

/// The full resolved route chain for the current path.
///
/// Built once per navigation, consumed by outlets during rendering.
/// Each outlet reads its entry by depth index.
#[derive(Debug, Clone, Default)]
pub struct MatchStack {
    entries: Vec<MatchEntry>,
}

impl MatchStack {
    /// Create an empty match stack.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Return the entry at `depth`, or `None` if out of range.
    pub fn at_depth(&self, depth: usize) -> Option<&MatchEntry> {
        self.entries.get(depth)
    }

    /// Return the root (depth 0) entry, or `None` if the stack is empty.
    pub fn root(&self) -> Option<&MatchEntry> {
        self.entries.first()
    }

    /// Return the leaf (deepest) entry, or `None` if the stack is empty.
    pub fn leaf(&self) -> Option<&MatchEntry> {
        self.entries.last()
    }

    /// Return the total number of matched levels in the stack.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if no routes matched the path.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the maximum depth (0-indexed), or `None` if the stack is empty.
    pub fn max_depth(&self) -> Option<usize> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.len() - 1)
        }
    }

    /// Return all entries as a slice (ordered root → leaf).
    pub fn entries(&self) -> &[MatchEntry] {
        &self.entries
    }

    /// Return the accumulated params at the deepest matched level.
    pub fn params(&self) -> RouteParams {
        self.leaf().map(|e| e.params.clone()).unwrap_or_default()
    }

    /// Return `true` if the stack contains an entry at the given `depth`.
    pub fn has_depth(&self, depth: usize) -> bool {
        depth < self.entries.len()
    }

    /// Return a multi-line human-readable representation (debug builds only).
    #[cfg(debug_assertions)]
    pub fn debug_string(&self) -> String {
        if self.entries.is_empty() {
            return "MatchStack: (empty)".to_string();
        }

        let mut lines = vec!["MatchStack:".to_string()];
        for entry in &self.entries {
            let indent = "  ".repeat(entry.depth);
            let params_str = if entry.params.is_empty() {
                String::new()
            } else {
                format!(
                    " params={{{}}}",
                    entry
                        .params
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            lines.push(format!(
                "{}[{}] Route(\"{}\"){}",
                indent, entry.depth, entry.route.config.path, params_str
            ));
        }
        lines.join("\n")
    }
}

// ============================================================================
// Resolution Algorithm
// ============================================================================

/// Maximum nesting depth to prevent infinite recursion
const MAX_DEPTH: usize = 16;

/// Resolve the full match stack for a given path against the route tree.
///
/// This is called once per navigation and produces a `MatchStack` that
/// outlets consume during rendering.
///
/// # Algorithm
///
/// 1. Split path into segments: `/dashboard/settings/profile` → `["dashboard", "settings", "profile"]`
/// 2. Try each top-level route against the first segment(s)
/// 3. On match, consume segments and recurse into children
/// 4. At each level, push a `MatchEntry` into the stack
/// 5. When segments exhausted, try index route (empty path child)
///
/// # Examples
///
/// ```ignore
/// use gpui_navigator::resolve::resolve_match_stack;
///
/// let stack = resolve_match_stack(&routes, "/dashboard/settings/profile");
/// assert_eq!(stack.len(), 4); // root, dashboard, settings, profile
/// ```
pub fn resolve_match_stack(routes: &[Arc<Route>], path: &str) -> MatchStack {
    let normalized = normalize_path(path);
    let path_str = normalized.trim_start_matches('/').trim_end_matches('/');

    let segments: Vec<&str> = if path_str.is_empty() {
        vec![]
    } else {
        path_str.split('/').collect()
    };

    let mut stack = MatchStack::new();
    resolve_recursive(routes, &segments, 0, &RouteParams::new(), &mut stack);

    if stack.is_empty() {
        warn_log!("No route matched path '{}'", path);
    } else {
        debug_log!(
            "Resolved path '{}' → {} levels: [{}]",
            path,
            stack.len(),
            stack
                .entries
                .iter()
                .map(|e| format!("\"{}\"", e.route.config.path))
                .collect::<Vec<_>>()
                .join(" → ")
        );
    }

    stack
}

/// Recursive route matching with backtracking.
///
/// Returns `true` if a complete match was found (all segments consumed or
/// a valid leaf/index route was reached).
fn resolve_recursive(
    routes: &[Arc<Route>],
    remaining: &[&str],
    depth: usize,
    inherited_params: &RouteParams,
    stack: &mut MatchStack,
) -> bool {
    // Safety: prevent infinite recursion
    if depth >= MAX_DEPTH {
        warn_log!(
            "Maximum route nesting depth ({}) exceeded. Check for circular routes.",
            MAX_DEPTH
        );
        return false;
    }

    for route in routes {
        let route_path = route
            .config
            .path
            .trim_start_matches('/')
            .trim_end_matches('/');

        let route_segments: Vec<&str> = if route_path.is_empty() {
            vec![]
        } else {
            route_path.split('/').collect()
        };

        trace_log!(
            "Trying route '{}' at depth {} ({} remaining segments)",
            route_path,
            depth,
            remaining.len()
        );

        // === Try to match this route's segments ===

        // Case 1: Route has an empty path (index/layout route)
        if route_segments.is_empty() {
            let params = inherited_params.clone();

            // Empty-path route with children = layout route (matches anything)
            // Empty-path route without children = index route (matches only when no segments left)
            if remaining.is_empty() {
                // No segments left → this is an index/layout match
                stack.entries.push(MatchEntry {
                    route: Arc::clone(route),
                    params: params.clone(),
                    depth,
                });

                // If layout with children, try to resolve index child
                if !route.children.is_empty() {
                    try_index_route(&route.children, depth + 1, &params, stack);
                }
                return true;
            }

            // Segments remain and route has children → layout route wrapping children
            if !route.children.is_empty() {
                stack.entries.push(MatchEntry {
                    route: Arc::clone(route),
                    params: params.clone(),
                    depth,
                });

                if resolve_recursive(&route.children, remaining, depth + 1, &params, stack) {
                    return true;
                }

                // Children didn't match → backtrack
                stack.entries.pop();
            }

            continue;
        }

        // Case 2: Route has path segments → try to match against remaining path
        if route_segments.len() > remaining.len() {
            continue; // Not enough path segments
        }

        let mut params = inherited_params.clone();
        let mut matched = true;

        for (i, route_seg) in route_segments.iter().enumerate() {
            if route_seg.starts_with(':') {
                // Parameter segment → extract value
                let param_name = route_seg.trim_start_matches(':');
                // Strip constraint syntax: `:id<i32>` → `id`
                let param_name = if let Some(pos) = param_name.find('<') {
                    &param_name[..pos]
                } else {
                    param_name
                };
                params.insert(param_name.to_string(), remaining[i].to_string());
            } else if *route_seg == remaining[i] {
                // Static segment → exact match
            } else {
                matched = false;
                break;
            }
        }

        if !matched {
            continue;
        }

        // Segments matched! Push entry.
        let consumed = route_segments.len();
        let after = &remaining[consumed..];

        trace_log!(
            "Matched route '{}' at depth {}, params: {:?}",
            route_path,
            depth,
            params.all()
        );

        stack.entries.push(MatchEntry {
            route: Arc::clone(route),
            params: params.clone(),
            depth,
        });

        if after.is_empty() {
            // All segments consumed
            if !route.children.is_empty() {
                // Has children → try to resolve index child
                try_index_route(&route.children, depth + 1, &params, stack);
            }
            return true;
        }

        // More segments remain → recurse into children
        if !route.children.is_empty()
            && resolve_recursive(&route.children, after, depth + 1, &params, stack)
        {
            return true;
        }

        // No children matched (or no children) → backtrack
        trace_log!(
            "Backtracking from route '{}' at depth {}",
            route_path,
            depth
        );
        stack.entries.pop();
    }

    false
}

/// Try to find and push an index route (empty path or "index" path child).
///
/// Called when all path segments are consumed but the current route has children.
/// This ensures navigating to `/dashboard` renders the default child.
fn try_index_route(
    children: &[Arc<Route>],
    depth: usize,
    params: &RouteParams,
    stack: &mut MatchStack,
) {
    // Priority 1: Empty path child
    for child in children {
        let child_path = child
            .config
            .path
            .trim_start_matches('/')
            .trim_end_matches('/');

        if child_path.is_empty() {
            trace_log!("Index route (empty path) resolved at depth {}", depth);
            stack.entries.push(MatchEntry {
                route: Arc::clone(child),
                params: params.clone(),
                depth,
            });

            // Recursively check if index route also has children with index
            if !child.children.is_empty() {
                try_index_route(&child.children, depth + 1, params, stack);
            }
            return;
        }
    }

    // Priority 2: "index" named child
    for child in children {
        let child_path = child
            .config
            .path
            .trim_start_matches('/')
            .trim_end_matches('/');

        if child_path == "index" {
            trace_log!("Index route ('index') resolved at depth {}", depth);
            stack.entries.push(MatchEntry {
                route: Arc::clone(child),
                params: params.clone(),
                depth,
            });
            return;
        }
    }

    trace_log!(
        "No index route among {} children at depth {}",
        children.len(),
        depth
    );
}

// ============================================================================
// Named Outlet Resolution
// ============================================================================

/// Resolve a named outlet at a given depth.
///
/// Named outlets use `Route::named_children` instead of `Route::children`.
/// The match stack doesn't include named outlet entries — they are resolved
/// on demand by the named outlet during rendering.
///
/// Returns the first matching child from the named outlet's children.
pub fn resolve_named_outlet(
    match_stack: &MatchStack,
    outlet_depth: usize,
    outlet_name: &str,
    current_path: &str,
) -> Option<(Arc<Route>, RouteParams)> {
    // The parent route is at outlet_depth - 1 in the stack
    let parent_depth = outlet_depth.checked_sub(1)?;
    let parent_entry = match_stack.at_depth(parent_depth)?;

    // Get named children for this outlet
    let named_children = parent_entry.route.get_named_children(outlet_name)?;

    if named_children.is_empty() {
        return None;
    }

    // For named outlets, resolve against remaining path segments
    let normalized = normalize_path(current_path);
    let path_str = normalized.trim_start_matches('/').trim_end_matches('/');
    let all_segments: Vec<&str> = if path_str.is_empty() {
        vec![]
    } else {
        path_str.split('/').collect()
    };

    // Calculate how many segments the parent chain consumed
    let consumed = count_consumed_segments(match_stack, parent_depth);
    let remaining = &all_segments[consumed.min(all_segments.len())..];

    // Try to match a named child
    let params = parent_entry.params.clone();

    for child in named_children {
        let child_path = child
            .config
            .path
            .trim_start_matches('/')
            .trim_end_matches('/');

        if child_path.is_empty() {
            // Index route for named outlet
            return Some((Arc::clone(child), params));
        }

        if remaining.is_empty() {
            continue;
        }

        // Simple single-segment match (named outlets are typically flat)
        if child_path == remaining[0] || child_path.starts_with(':') {
            #[allow(clippy::redundant_clone)]
            let mut child_params = params.clone();
            if child_path.starts_with(':') {
                let name = child_path.trim_start_matches(':');
                child_params.insert(name.to_string(), remaining[0].to_string());
            }
            return Some((Arc::clone(child), child_params));
        }
    }

    // Default: first child with empty path (if any)
    for child in named_children {
        let p = child
            .config
            .path
            .trim_start_matches('/')
            .trim_end_matches('/');
        if p.is_empty() {
            return Some((Arc::clone(child), params));
        }
    }

    None
}

/// Count how many path segments the match stack consumed up to a given depth.
fn count_consumed_segments(stack: &MatchStack, up_to_depth: usize) -> usize {
    let mut count = 0;
    for entry in stack.entries().iter().take(up_to_depth + 1) {
        let path = entry
            .route
            .config
            .path
            .trim_start_matches('/')
            .trim_end_matches('/');
        if !path.is_empty() {
            count += path.split('/').count();
        }
    }
    count
}

// Tests moved to tests/unit/resolve.rs to avoid compiler stack overflow
// when compiling all tests in a single compilation unit.
