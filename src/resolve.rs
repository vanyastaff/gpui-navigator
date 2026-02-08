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
use crate::RouteParams;
use std::cell::Cell;
use std::sync::Arc;

// ============================================================================
// Depth Tracking (thread-local)
// ============================================================================

thread_local! {
    /// Current outlet depth during rendering.
    /// Set by `router_view` (to 0) and incremented by each outlet.
    static OUTLET_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Reset outlet depth to 0. Called at the start of `router_view`.
pub fn reset_outlet_depth() {
    OUTLET_DEPTH.with(|d| d.set(0));
}

/// Claim the next outlet depth. Returns the depth this outlet should render.
///
/// Called by `render_router_outlet()` and `RouterOutlet::render()`.
/// The caller should render `match_stack[returned_depth]`.
pub fn claim_outlet_depth() -> usize {
    OUTLET_DEPTH.with(|d| {
        let current = d.get();
        let next = current + 1;
        d.set(next);
        next
    })
}

/// Set outlet depth explicitly. Used by `RouterOutlet::render()` to
/// ensure nested outlets created inside its builder get the correct depth.
pub fn set_outlet_depth(depth: usize) {
    OUTLET_DEPTH.with(|d| d.set(depth));
}

/// Get current outlet depth without modifying it.
pub fn current_outlet_depth() -> usize {
    OUTLET_DEPTH.with(|d| d.get())
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
    /// Create an empty match stack
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Get entry at a specific depth
    pub fn at_depth(&self, depth: usize) -> Option<&MatchEntry> {
        self.entries.get(depth)
    }

    /// Get the root (depth 0) entry
    pub fn root(&self) -> Option<&MatchEntry> {
        self.entries.first()
    }

    /// Get the leaf (deepest) entry
    pub fn leaf(&self) -> Option<&MatchEntry> {
        self.entries.last()
    }

    /// Total number of matched levels
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty (no routes matched)
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Maximum depth (0-indexed). Returns None if empty.
    pub fn max_depth(&self) -> Option<usize> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.len() - 1)
        }
    }

    /// Get all entries as a slice
    pub fn entries(&self) -> &[MatchEntry] {
        &self.entries
    }

    /// Get the accumulated params at the deepest level
    pub fn params(&self) -> RouteParams {
        self.leaf().map(|e| e.params.clone()).unwrap_or_default()
    }

    /// Check if a specific depth has a valid entry
    pub fn has_depth(&self, depth: usize) -> bool {
        depth < self.entries.len()
    }

    /// Pretty-print for debugging
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

    #[cfg(debug_assertions)]
    {
        crate::debug_log!(
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
        crate::warn_log!(
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
            stack.entries.push(MatchEntry {
                route: Arc::clone(child),
                params: params.clone(),
                depth,
            });
            return;
        }
    }
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
