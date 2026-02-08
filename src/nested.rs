//! Nested route resolution
//!
//! This module provides functionality for resolving child routes in nested routing scenarios.
//! The cache functionality has been moved to the `cache` module (available with `cache` feature).
//!
//! # Path Normalization (T053)
//!
//! All path operations in this module use consistent normalization to handle various path formats:
//!
//! ## Normalization Rules
//!
//! 1. **Empty paths** are normalized to `"/"` (root)
//! 2. **Leading slashes** are ensured (e.g., `"dashboard"` → `"/dashboard"`)
//! 3. **Trailing slashes** are removed (except for root: `"/"`)
//! 4. **Multiple slashes** are collapsed to single slash (e.g., `"//dashboard"` → `"/dashboard"`)
//! 5. **Root variations** (`"/"`, `"//"`, `""`) all normalize to `"/"`
//!
//! ## Examples
//!
//! ```ignore
//! // All these paths resolve to the same route:
//! navigate("/dashboard");
//! navigate("dashboard");
//! navigate("/dashboard/");
//! navigate("//dashboard");
//!
//! // Root path variations:
//! navigate("/");     // Root
//! navigate("");      // Also root
//! navigate("//");    // Also root
//! ```
//!
//! ## Implementation
//!
//! Path normalization is performed by the [`normalize_path()`] function, which returns
//! `Cow<str>` to avoid allocations when paths are already normalized. This is critical
//! for performance in hot paths like route resolution.

use crate::route::Route;
use crate::{trace_log, warn_log, RouteParams};
use std::borrow::Cow;
use std::sync::Arc;

/// Maximum recursion depth for nested routes (T031 - User Story 3)
///
/// Prevents infinite loops and stack overflow in deeply nested route hierarchies.
/// Configured for 10 levels to support complex applications while maintaining safety.
const MAX_RECURSION_DEPTH: usize = 10;

/// Resolved child route information
///
/// Contains the matched child route and merged parameters from parent and child.
pub type ResolvedChildRoute = (Arc<Route>, RouteParams);

/// Normalize a path for consistent comparison
///
/// Ensures paths have a leading slash and no trailing slash (unless root).
/// Returns `Cow<str>` to avoid allocation when path is already normalized.
///
/// # Examples
///
/// ```
/// use gpui_navigator::normalize_path;
///
/// assert_eq!(normalize_path("/dashboard"), "/dashboard");
/// assert_eq!(normalize_path("dashboard"), "/dashboard");
/// assert_eq!(normalize_path("/dashboard/"), "/dashboard");
/// assert_eq!(normalize_path("/"), "/");
/// assert_eq!(normalize_path(""), "/");
/// ```
pub fn normalize_path(path: &'_ str) -> Cow<'_, str> {
    // Handle empty path -> "/"
    if path.is_empty() {
        return Cow::Borrowed("/");
    }

    // Handle already-normalized root
    if path == "/" {
        return Cow::Borrowed(path);
    }

    let has_leading = path.starts_with('/');
    let has_trailing = path.ends_with('/');

    // Already normalized: has leading, no trailing
    if has_leading && !has_trailing {
        return Cow::Borrowed(path);
    }

    // Need to normalize
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        Cow::Borrowed("/")
    } else {
        Cow::Owned(format!("/{}", trimmed))
    }
}

/// Extract parameter name from a route path segment
///
/// Strips leading ':' and any type constraints like `:id<i32>` -> `id`.
/// Returns `Cow<str>` to avoid allocation when no constraint exists.
///
/// # Examples
///
/// ```
/// use gpui_navigator::extract_param_name;
///
/// assert_eq!(extract_param_name(":id"), "id");
/// assert_eq!(extract_param_name(":id<i32>"), "id");
/// assert_eq!(extract_param_name(":user_id<uuid>"), "user_id");
/// ```
pub fn extract_param_name(segment: &'_ str) -> Cow<'_, str> {
    let without_colon = segment.trim_start_matches(':');

    // Check for constraint delimiter '<'
    if let Some(pos) = without_colon.find('<') {
        Cow::Owned(without_colon[..pos].to_string())
    } else {
        Cow::Borrowed(without_colon)
    }
}

/// Resolve a child route with recursion depth tracking (T031)
///
/// Public wrapper that starts recursion depth tracking at 0.
/// Use this function for all external calls.
pub fn resolve_child_route(
    parent_route: &Arc<Route>,
    current_path: &str,
    parent_params: &RouteParams,
    outlet_name: Option<&str>,
) -> Option<ResolvedChildRoute> {
    resolve_child_route_impl(parent_route, current_path, parent_params, outlet_name, 0)
}

/// Internal implementation with recursion depth tracking (T031)
///
/// Prevents infinite loops by enforcing MAX_RECURSION_DEPTH limit.
/// Returns None if depth exceeded.
fn resolve_child_route_impl(
    parent_route: &Arc<Route>,
    current_path: &str,
    parent_params: &RouteParams,
    outlet_name: Option<&str>,
    depth: usize,
) -> Option<ResolvedChildRoute> {
    // T031: Check recursion depth limit
    if depth >= MAX_RECURSION_DEPTH {
        warn_log!(
            "Maximum recursion depth ({}) exceeded while resolving path '{}' from parent '{}'",
            MAX_RECURSION_DEPTH,
            current_path,
            parent_route.config.path
        );
        return None;
    }
    // T051: Explicit check for empty current path - normalize to root
    let normalized_current = if current_path.is_empty() {
        "/"
    } else {
        current_path
    };

    // T050: Explicit check for root path - should match index route
    let is_root_path = normalized_current == "/";

    trace_log!(
        "resolve_child_route: parent='{}', current_path='{}' (normalized='{}', is_root={}), children={}, outlet_name={:?}",
        parent_route.config.path,
        current_path,
        normalized_current,
        is_root_path,
        parent_route.get_children().len(),
        outlet_name
    );
    // Get the children for this outlet (named or default)
    let children = if let Some(name) = outlet_name {
        // Named outlet - get children from named_children map
        match parent_route.get_named_children(name) {
            Some(named_children) => {
                trace_log!(
                    "Using named outlet '{}' with {} children",
                    name,
                    named_children.len()
                );
                named_children
            }
            None => {
                // T060: Improved error message with available outlets list
                let available_outlets: Vec<&str> = parent_route.named_outlet_names();
                if available_outlets.is_empty() {
                    warn_log!(
                        "Named outlet '{}' not found in route '{}'. No named outlets are defined for this route.",
                        name,
                        parent_route.config.path
                    );
                } else {
                    warn_log!(
                        "Named outlet '{}' not found in route '{}'. Available named outlets: {:?}",
                        name,
                        parent_route.config.path,
                        available_outlets
                    );
                }
                return None;
            }
        }
    } else {
        // Default outlet - use regular children
        parent_route.get_children()
    };

    if children.is_empty() {
        trace_log!("No children found for outlet {:?}", outlet_name);
        return None;
    }

    // Extract parent path from the route config
    let parent_path = &parent_route.config.path;

    // Normalize paths for comparison using helper function
    let parent_path_normalized = normalize_path(parent_path);
    let current_path_normalized = normalize_path(normalized_current);

    // Check if current path starts with parent path (BUG-001: fixed double normalization)
    let parent_without_slash = parent_path_normalized.trim_start_matches('/');
    let current_without_slash = current_path_normalized.trim_start_matches('/');

    // Special handling for parameter routes - they don't have static prefixes to strip
    let remaining = if parent_without_slash.starts_with(':') {
        // Parent is a parameter route - remaining path is the current path itself
        // (parameter routes don't have prefixes to strip)
        current_without_slash.to_string()
    } else {
        // Parent is a static route - check prefix and extract remaining
        if !current_without_slash.starts_with(parent_without_slash) {
            return None;
        }

        if parent_without_slash.is_empty() {
            // Parent is root ("/"), remaining is the full current path
            current_without_slash.to_string()
        } else {
            // Parent has a path, strip it from current path
            current_without_slash
                .strip_prefix(parent_without_slash)
                .unwrap_or("")
                .trim_start_matches('/')
                .to_string()
        }
    };

    trace_log!(
        "  normalized: parent='{}', current='{}', remaining='{}', parent_without_slash='{}'",
        parent_path_normalized,
        current_path_normalized,
        remaining,
        parent_without_slash
    );

    if remaining.is_empty() {
        // No child path, look for index route
        return find_index_route(children, parent_params.clone());
    }

    // Split remaining path into segments
    let segments: Vec<&str> = remaining.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return find_index_route(children, parent_params.clone());
    }

    let first_segment = segments[0];
    trace_log!("  first_segment: '{}'", first_segment);

    // Try to match first segment against child routes
    for child in children {
        let child_path = child.config.path.trim_start_matches('/');

        // Check for exact match or parameter match
        if child_path == first_segment || child_path.starts_with(':') {
            trace_log!("  matched: '{}'", child_path);
            // Found matching child!
            let mut combined_params = parent_params.clone();

            // If this is a parameter route, extract the parameter (BUG-003: use extract_param_name)
            if child_path.starts_with(':') {
                let param_name = extract_param_name(child_path);

                // T047: Warn on parameter collision (debug mode)
                #[cfg(debug_assertions)]
                if parent_params.contains(&param_name) {
                    warn_log!(
                        "Parameter collision: child route '{}' shadows parent parameter '{}' (parent value: '{}', child value: '{}')",
                        child.config.path,
                        param_name,
                        parent_params.get(&param_name).map_or("<none>", String::as_str),
                        first_segment
                    );
                }

                combined_params.insert(param_name.to_string(), first_segment.to_string());
            }

            // BUG-002: Handle nested parameters in deeper child paths (recursive resolution)
            if segments.len() > 1 {
                // More segments remaining - recursively resolve deeper levels
                trace_log!("  recursing for remaining {} segments", segments.len() - 1);

                // Construct path for recursive call:
                // - For parameter routes: pass remaining segments only (parameter has no prefix)
                // - For static routes: include the matched segment (so it can strip its own prefix)
                let remaining_path = if child_path.starts_with(':') {
                    // Parameter route - pass segments after the matched one
                    format!("/{}", segments[1..].join("/"))
                } else {
                    // Static route - include the matched segment so it can strip its prefix
                    format!("/{}", segments.join("/"))
                };
                trace_log!("  remaining_path for recursion: '{}'", remaining_path);

                // Recursively resolve the child route with the remaining path (T031: pass depth + 1)
                if let Some((grandchild, grandchild_params)) = resolve_child_route_impl(
                    child,
                    &remaining_path,
                    &combined_params,
                    outlet_name,
                    depth + 1,
                ) {
                    return Some((grandchild, grandchild_params));
                }
                // If recursive resolution fails, continue to next child
                continue;
            }

            return Some((Arc::clone(child), combined_params));
        }
    }

    None
}

/// Find an index route (default child route when no specific child is selected)
///
/// T037: Prioritize index routes (empty path "") when no exact child match.
/// An index route serves as the default content when navigating to a parent path.
///
/// # Priority Order
///
/// 1. Empty path ("") - highest priority, explicit index route
/// 2. Path "index" - alternative naming convention
///
/// # Examples
///
/// ```ignore
/// // Define index route
/// Route::new("/dashboard", |_, _, _| {
///     div().child(render_router_outlet(...))
/// })
/// .children(vec![
///     Route::new("", |_, _, _| div().child("Overview")).into(),  // Index route
///     Route::new("settings", |_, _, _| div().child("Settings")).into(),
/// ]);
///
/// // Navigate to "/dashboard" → renders Overview (index route)
/// // Navigate to "/dashboard/settings" → renders Settings
/// ```
fn find_index_route(children: &[Arc<Route>], params: RouteParams) -> Option<ResolvedChildRoute> {
    trace_log!("find_index_route: searching {} children", children.len());

    // T037: Prioritize index routes by checking for empty path first
    // Priority 1: Empty path ("") - explicit index route
    for child in children {
        let child_path_normalized = normalize_path(&child.config.path);
        let child_path = child_path_normalized.trim_matches('/');

        trace_log!(
            "find_index_route: checking child path: '{}' (original: '{}', normalized: '{}')",
            child_path,
            child.config.path,
            child_path_normalized
        );

        if child_path.is_empty() {
            trace_log!(
                "find_index_route: ✓ found index route with empty path '{}'",
                child.config.path
            );
            return Some((Arc::clone(child), params));
        }
    }

    // Priority 2: Path "index" - alternative naming convention
    for child in children {
        let child_path_normalized = normalize_path(&child.config.path);
        let child_path = child_path_normalized.trim_matches('/');

        if child_path == "index" {
            trace_log!(
                "find_index_route: ✓ found index route with path 'index' (original: '{}')",
                child.config.path
            );
            return Some((Arc::clone(child), params));
        }
    }

    trace_log!(
        "find_index_route: ✗ no index route found among {} children",
        children.len()
    );
    None
}

/// Build the full path for a child route
///
/// Combines parent and child paths into a complete route path.
///
/// Returns `Cow<str>` to avoid unnecessary allocations when possible.
/// Uses borrowed string when no modification is needed.
///
/// # Example
///
/// ```
/// use gpui_navigator::build_child_path;
///
/// let full_path = build_child_path("/dashboard", "settings");
/// assert_eq!(full_path, "/dashboard/settings");
/// ```
pub fn build_child_path<'a>(parent_path: &'a str, child_path: &'a str) -> Cow<'a, str> {
    // CRITICAL: Don't normalize empty child paths - they represent index routes
    // Normalizing "" to "/" would make the child have the same path as parent, causing infinite recursion

    // For empty child path (index route), return the parent path as-is
    if child_path.is_empty() {
        return normalize_path(parent_path);
    }

    // For non-empty paths, normalize them
    let parent_normalized = normalize_path(parent_path);
    let child_normalized = normalize_path(child_path);

    let parent = parent_normalized.trim_matches('/');
    let child = child_normalized.trim_matches('/');

    if parent.is_empty() {
        // T052: Root path handling - when parent is root ("/"), child becomes the full path
        child_normalized
    } else {
        // Combine parent and child
        Cow::Owned(format!("/{}/{}", parent, child))
    }
}
