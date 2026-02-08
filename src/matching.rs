//! Segment-based route matching for nested routing
//!
//! This module provides simplified segment-based path matching specifically
//! designed for the redesigned nested routing architecture. Unlike the
//! priority-based matcher in matcher.rs, this focuses on straightforward
//! parent-child hierarchical matching.
//!
//! # Design
//!
//! - Split paths by '/' into segments
//! - Match literal segments exactly
//! - Extract `:param` segments into RouteParams
//! - Support for empty path "" (index routes)
//! - No regex or complex patterns (use matcher.rs for that)

use crate::params::RouteParams;
use crate::route::Route;
use std::sync::Arc;

/// Result of matching a path against a route
#[derive(Debug, Clone)]
pub struct RouteMatch<'a> {
    /// The matched route
    pub route: &'a Arc<Route>,
    /// Extracted route parameters
    pub params: RouteParams,
    /// Remaining unmatched path segments (for nested resolution)
    pub remaining: Vec<String>,
}

/// Match a path against a route pattern, extracting parameters
///
/// # Examples
///
/// ```ignore
/// let route = Arc::new(Route::new("/user/:id", builder));
/// let result = match_path("/user/123/profile", &route);
///
/// assert!(result.is_some());
/// let m = result.unwrap();
/// assert_eq!(m.params.get("id"), Some(&"123".to_string()));
/// assert_eq!(m.remaining, vec!["profile"]);
/// ```
pub fn match_path<'a>(path: &str, route: &'a Arc<Route>) -> Option<RouteMatch<'a>> {
    // T032: Early exit optimizations for performance

    // Quick check: empty path only matches empty route
    if path.is_empty() || path == "/" {
        let route_path = route.config.path.trim_matches('/');
        if !route_path.is_empty() {
            return None;
        }
    }

    let path_segments = split_path(path);
    let route_segments = split_path(&route.config.path);

    // T032: Cache segment counts for multiple comparisons
    let path_len = path_segments.len();
    let route_len = route_segments.len();

    // Early exit: if route has more segments than path, can't match
    // (unless route has trailing params that can be empty)
    if route_len > path_len {
        return None;
    }

    // T032: Early exit if both empty - this is a match
    if route_len == 0 && path_len == 0 {
        return Some(RouteMatch {
            route,
            params: RouteParams::new(),
            remaining: vec![],
        });
    }

    let mut params = RouteParams::new();
    let mut matched_count = 0;

    // Match each route segment against path segments
    for (route_seg, path_seg) in route_segments.iter().zip(path_segments.iter()) {
        if route_seg.starts_with(':') {
            // Parameter segment: extract param name and value
            let param_name = &route_seg[1..];
            params.set(param_name.to_string(), path_seg.clone());
            matched_count += 1;
        } else if route_seg == path_seg {
            // Literal segment: must match exactly
            matched_count += 1;
        } else {
            // Mismatch
            return None;
        }
    }

    // Calculate remaining unmatched segments
    let remaining = path_segments[matched_count..].to_vec();

    Some(RouteMatch {
        route,
        params,
        remaining,
    })
}

/// Split a path into segments, filtering empty segments
///
/// # Examples
///
/// ```ignore
/// assert_eq!(split_path("/users/123"), vec!["users", "123"]);
/// assert_eq!(split_path("/"), vec![]);
/// assert_eq!(split_path(""), vec![]);
/// assert_eq!(split_path("/users/"), vec!["users"]);
/// ```
pub fn split_path(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Extract parameter name from a route segment
///
/// # Examples
///
/// ```ignore
/// assert_eq!(extract_param_name(":id"), Some("id"));
/// assert_eq!(extract_param_name("users"), None);
/// ```
pub fn extract_param_name(segment: &str) -> Option<&str> {
    if segment.starts_with(':') {
        Some(&segment[1..])
    } else {
        None
    }
}

/// Check if a route segment is a parameter
pub fn is_param_segment(segment: &str) -> bool {
    segment.starts_with(':')
}

/// Check if a route segment is a wildcard
pub fn is_wildcard_segment(segment: &str) -> bool {
    segment == "*" || segment.starts_with('*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_path() {
        assert_eq!(split_path("/users/123"), vec!["users", "123"]);
        assert_eq!(
            split_path("/users/123/profile"),
            vec!["users", "123", "profile"]
        );
        assert_eq!(split_path("/"), Vec::<String>::new());
        assert_eq!(split_path(""), Vec::<String>::new());
        assert_eq!(split_path("/users/"), vec!["users"]);
        assert_eq!(split_path("users"), vec!["users"]);
    }

    #[test]
    fn test_extract_param_name() {
        assert_eq!(extract_param_name(":id"), Some("id"));
        assert_eq!(extract_param_name(":userId"), Some("userId"));
        assert_eq!(extract_param_name("users"), None);
        assert_eq!(extract_param_name(""), None);
    }

    #[test]
    fn test_is_param_segment() {
        assert!(is_param_segment(":id"));
        assert!(is_param_segment(":userId"));
        assert!(!is_param_segment("users"));
        assert!(!is_param_segment(""));
    }

    #[test]
    fn test_is_wildcard_segment() {
        assert!(is_wildcard_segment("*"));
        assert!(is_wildcard_segment("*path"));
        assert!(!is_wildcard_segment("path"));
        assert!(!is_wildcard_segment(":id"));
    }
}
