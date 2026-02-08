//! Route parameter extraction and query string parsing.
//!
//! This module provides two complementary types for working with URL data:
//!
//! - [`RouteParams`] — path parameters extracted from dynamic segments (e.g.
//!   `:id` in `/users/:id`). Supports typed access via [`get_as`](RouteParams::get_as),
//!   parent-child merging via [`merge`](RouteParams::merge), and extraction from
//!   raw paths via [`from_path`](RouteParams::from_path).
//! - [`QueryParams`] — query string parameters parsed from the `?key=value&...`
//!   portion of a URL. Supports multi-valued keys (e.g. `?tag=a&tag=b`), typed
//!   access, and round-trip serialization.
//!
//! # Example
//!
//! ```
//! use gpui_navigator::{RouteParams, QueryParams};
//!
//! // Path parameters from /users/42
//! let mut params = RouteParams::new();
//! params.set("id".to_string(), "42".to_string());
//! assert_eq!(params.get_as::<u32>("id"), Some(42));
//!
//! // Query parameters from ?page=1&sort=name
//! let query = QueryParams::from_query_string("page=1&sort=name");
//! assert_eq!(query.get_as::<u32>("page"), Some(1));
//! assert_eq!(query.get("sort"), Some(&"name".to_string()));
//! ```

use std::collections::HashMap;

/// Route parameters extracted from path segments
///
/// # Example
///
/// ```
/// use gpui_navigator::RouteParams;
///
/// // Route pattern: /users/:id
/// // Matched path: /users/123
/// let mut params = RouteParams::new();
/// params.insert("id".to_string(), "123".to_string());
///
/// assert_eq!(params.get("id"), Some(&"123".to_string()));
/// assert_eq!(params.get_as::<i32>("id"), Some(123));
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RouteParams {
    params: HashMap<String, String>,
}

impl RouteParams {
    /// Create empty route parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from an existing `HashMap`.
    pub fn from_map(params: HashMap<String, String>) -> Self {
        Self { params }
    }

    /// Get a parameter value by key.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.params.get(key)
    }

    /// Get a parameter and parse it as a specific type
    ///
    /// Returns `None` if the parameter doesn't exist or cannot be parsed.
    pub fn get_as<T>(&self, key: &str) -> Option<T>
    where
        T: std::str::FromStr,
    {
        self.params.get(key)?.parse().ok()
    }

    /// Insert or overwrite a parameter.
    pub fn insert(&mut self, key: String, value: String) {
        self.params.insert(key, value);
    }

    /// Set a parameter (alias for [`insert`](Self::insert)).
    pub fn set(&mut self, key: String, value: String) {
        self.params.insert(key, value);
    }

    /// Return `true` if the given key is present.
    pub fn contains(&self, key: &str) -> bool {
        self.params.contains_key(key)
    }

    /// Get a reference to the underlying parameter map.
    pub fn all(&self) -> &HashMap<String, String> {
        &self.params
    }

    /// Get a mutable reference to the underlying parameter map.
    pub fn all_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.params
    }

    /// Iterate over all `(key, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.params.iter()
    }

    /// Return `true` if there are no parameters.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Return the number of parameters.
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Merge parent parameters with child parameters
    ///
    /// Child parameters override parent parameters in case of collision.
    /// This is used for nested routing to inherit parent route parameters.
    ///
    /// # Example
    ///
    /// ```
    /// use gpui_navigator::RouteParams;
    ///
    /// let mut parent = RouteParams::new();
    /// parent.set("workspaceId".to_string(), "123".to_string());
    /// parent.set("view".to_string(), "list".to_string());
    ///
    /// let mut child = RouteParams::new();
    /// child.set("projectId".to_string(), "456".to_string());
    /// child.set("view".to_string(), "grid".to_string()); // Override parent
    ///
    /// let merged = RouteParams::merge(&parent, &child);
    /// assert_eq!(merged.get("workspaceId"), Some(&"123".to_string()));
    /// assert_eq!(merged.get("projectId"), Some(&"456".to_string()));
    /// assert_eq!(merged.get("view"), Some(&"grid".to_string())); // Child wins
    /// ```
    pub fn merge(parent: &RouteParams, child: &RouteParams) -> RouteParams {
        let mut merged = parent.clone();

        // Child params override parent params
        for (key, value) in child.iter() {
            merged.insert(key.clone(), value.clone());
        }

        merged
    }

    /// Extract route parameters from a path given a pattern
    ///
    /// T045: Helper function for User Story 5 - Parameter Inheritance.
    /// Matches a path against a pattern and extracts parameter values.
    ///
    /// # Pattern Syntax
    ///
    /// - `:paramName` - Dynamic segment that matches any value
    /// - `literal` - Static segment that must match exactly
    ///
    /// # Example
    ///
    /// ```
    /// use gpui_navigator::RouteParams;
    ///
    /// // Pattern: /users/:userId/posts/:postId
    /// // Path: /users/123/posts/456
    /// let params = RouteParams::from_path("/users/123/posts/456", "/users/:userId/posts/:postId");
    ///
    /// assert_eq!(params.get("userId"), Some(&"123".to_string()));
    /// assert_eq!(params.get("postId"), Some(&"456".to_string()));
    ///
    /// // No match returns empty params
    /// let params = RouteParams::from_path("/products/xyz", "/users/:userId");
    /// assert!(params.is_empty());
    /// ```
    pub fn from_path(path: &str, pattern: &str) -> RouteParams {
        let mut params = RouteParams::new();

        // Split path and pattern into segments
        let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let pattern_segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();

        // Must have same number of segments
        if path_segments.len() != pattern_segments.len() {
            return params;
        }

        // Match each segment
        for (path_seg, pattern_seg) in path_segments.iter().zip(pattern_segments.iter()) {
            if let Some(param_name) = pattern_seg.strip_prefix(':') {
                // Dynamic segment - extract parameter
                // Handle type constraints like :id<i32> -> extract "id"
                let param_name = if let Some(pos) = param_name.find('<') {
                    &param_name[..pos]
                } else {
                    param_name
                };
                params.insert(param_name.to_string(), path_seg.to_string());
            } else if pattern_seg != path_seg {
                // Static segment mismatch - no match
                return RouteParams::new();
            }
        }

        params
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Route parameters tests

    #[test]
    fn test_route_params_basic() {
        let mut params = RouteParams::new();
        params.insert("id".to_string(), "123".to_string());

        assert_eq!(params.get("id"), Some(&"123".to_string()));
        assert!(params.contains("id"));
        assert!(!params.contains("missing"));
    }

    #[test]
    fn test_route_params_get_as() {
        let mut params = RouteParams::new();
        params.insert("id".to_string(), "123".to_string());
        params.insert("active".to_string(), "true".to_string());

        assert_eq!(params.get_as::<i32>("id"), Some(123));
        assert_eq!(params.get_as::<u32>("id"), Some(123));
        assert_eq!(params.get_as::<bool>("active"), Some(true));
        assert_eq!(params.get_as::<i32>("missing"), None);
    }

    #[test]
    fn test_route_params_from_map() {
        let mut map = HashMap::new();
        map.insert("name".to_string(), "John".to_string());
        map.insert("age".to_string(), "30".to_string());

        let params = RouteParams::from_map(map);

        assert_eq!(params.get("name"), Some(&"John".to_string()));
        assert_eq!(params.get_as::<i32>("age"), Some(30));
    }

    #[test]
    fn test_route_params_set() {
        let mut params = RouteParams::new();
        params.set("key".to_string(), "value".to_string());

        assert_eq!(params.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_route_params_all() {
        let mut params = RouteParams::new();
        params.insert("a".to_string(), "1".to_string());
        params.insert("b".to_string(), "2".to_string());

        let all = params.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all.get("a"), Some(&"1".to_string()));
    }

    #[test]
    fn test_route_params_iter() {
        let mut params = RouteParams::new();
        params.insert("x".to_string(), "1".to_string());
        params.insert("y".to_string(), "2".to_string());

        let count = params.iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_route_params_empty() {
        let params = RouteParams::new();
        assert!(params.is_empty());
        assert_eq!(params.len(), 0);

        let mut params = RouteParams::new();
        params.insert("key".to_string(), "value".to_string());
        assert!(!params.is_empty());
        assert_eq!(params.len(), 1);
    }
}

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters parsed from URL query string
///
/// Supports multiple values for the same key.
///
/// # Example
///
/// ```
/// use gpui_navigator::QueryParams;
///
/// let query = QueryParams::from_query_string("page=1&sort=name&tag=rust&tag=gpui");
///
/// assert_eq!(query.get("page"), Some(&"1".to_string()));
/// assert_eq!(query.get_as::<i32>("page"), Some(1));
/// assert_eq!(query.get_all("tag").unwrap().len(), 2);
/// ```
#[derive(Debug, Clone, Default)]
pub struct QueryParams {
    params: HashMap<String, Vec<String>>,
}

impl QueryParams {
    /// Create empty query parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse from query string
    ///
    /// # Example
    ///
    /// ```
    /// use gpui_navigator::QueryParams;
    ///
    /// let query = QueryParams::from_query_string("page=1&sort=name");
    /// assert_eq!(query.get("page"), Some(&"1".to_string()));
    /// ```
    pub fn from_query_string(query: &str) -> Self {
        let mut params = HashMap::new();

        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                // Simple URL decoding (replace %20 with space, etc.)
                let key = decode_uri_component(key);
                let value = decode_uri_component(value);

                params.entry(key).or_insert_with(Vec::new).push(value);
            }
        }

        Self { params }
    }

    /// Get the first value for a key.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.params.get(key)?.first()
    }

    /// Get all values for a key.
    ///
    /// Useful for parameters that can appear multiple times (e.g. `?tag=rust&tag=gpui`).
    pub fn get_all(&self, key: &str) -> Option<&Vec<String>> {
        self.params.get(key)
    }

    /// Get the first value for a key, parsed as type `T`.
    ///
    /// Returns `None` if the key is missing or the value cannot be parsed.
    pub fn get_as<T>(&self, key: &str) -> Option<T>
    where
        T: std::str::FromStr,
    {
        self.get(key)?.parse().ok()
    }

    /// Append a value for the given key.
    ///
    /// If the key already exists, the new value is added to the list (not replaced).
    pub fn insert(&mut self, key: String, value: String) {
        self.params.entry(key).or_default().push(value);
    }

    /// Return `true` if the given key is present.
    pub fn contains(&self, key: &str) -> bool {
        self.params.contains_key(key)
    }

    /// Serialize back into a query string.
    ///
    /// # Example
    ///
    /// ```
    /// use gpui_navigator::QueryParams;
    ///
    /// let mut query = QueryParams::new();
    /// query.insert("page".to_string(), "1".to_string());
    /// let s = query.to_query_string();
    /// assert!(s.contains("page=1"));
    /// ```
    pub fn to_query_string(&self) -> String {
        let pairs: Vec<String> = self
            .params
            .iter()
            .flat_map(|(key, values)| {
                values.iter().map(move |value| {
                    format!(
                        "{}={}",
                        encode_uri_component(key),
                        encode_uri_component(value)
                    )
                })
            })
            .collect();

        pairs.join("&")
    }

    /// Return `true` if there are no parameters.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Return the number of unique parameter keys.
    pub fn len(&self) -> usize {
        self.params.len()
    }
}

/// Simple URI component encoding (encode special characters)
fn encode_uri_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "%20".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

/// Simple URI component decoding
fn decode_uri_component(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Try to decode hex pair
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

// Query parameters tests

#[test]
fn test_query_params_basic() {
    let query = QueryParams::from_query_string("page=1&sort=name&filter=active");

    assert_eq!(query.get("page"), Some(&"1".to_string()));
    assert_eq!(query.get("sort"), Some(&"name".to_string()));
    assert_eq!(query.get("filter"), Some(&"active".to_string()));
    assert_eq!(query.get("missing"), None);
}

#[test]
fn test_query_params_get_as() {
    let query = QueryParams::from_query_string("page=1&limit=50&active=true");

    assert_eq!(query.get_as::<i32>("page"), Some(1));
    assert_eq!(query.get_as::<usize>("limit"), Some(50));
    assert_eq!(query.get_as::<bool>("active"), Some(true));
    assert_eq!(query.get_as::<i32>("missing"), None);
}

#[test]
fn test_query_params_multiple_values() {
    let query = QueryParams::from_query_string("tag=rust&tag=gpui&tag=ui");

    let tags = query.get_all("tag").unwrap();
    assert_eq!(tags.len(), 3);
    assert!(tags.contains(&"rust".to_string()));
    assert!(tags.contains(&"gpui".to_string()));
    assert!(tags.contains(&"ui".to_string()));

    // get() returns first value
    assert_eq!(query.get("tag"), Some(&"rust".to_string()));
}

#[test]
fn test_query_params_insert() {
    let mut query = QueryParams::new();
    query.insert("key".to_string(), "value1".to_string());
    query.insert("key".to_string(), "value2".to_string());

    let values = query.get_all("key").unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], "value1");
    assert_eq!(values[1], "value2");
}

#[test]
fn test_uri_encoding() {
    let encoded = encode_uri_component("hello world");
    assert_eq!(encoded, "hello%20world");

    let encoded = encode_uri_component("test@example.com");
    assert!(encoded.contains("%40")); // @ encoded as %40
}

#[test]
fn test_uri_decoding() {
    let decoded = decode_uri_component("hello%20world");
    assert_eq!(decoded, "hello world");

    let decoded = decode_uri_component("hello+world");
    assert_eq!(decoded, "hello world");
}

#[test]
fn test_to_query_string() {
    let mut query = QueryParams::new();
    query.insert("page".to_string(), "1".to_string());
    query.insert("sort".to_string(), "name".to_string());

    let s = query.to_query_string();
    // Order may vary, check both keys are present
    assert!(s.contains("page=1"));
    assert!(s.contains("sort=name"));
}

#[test]
fn test_query_params_empty() {
    let query = QueryParams::new();
    assert!(query.is_empty());
    assert_eq!(query.len(), 0);

    let mut query = QueryParams::new();
    query.insert("key".to_string(), "value".to_string());
    assert!(!query.is_empty());
    assert_eq!(query.len(), 1);
}

#[test]
fn test_empty_query_string() {
    let query = QueryParams::from_query_string("");
    assert!(query.is_empty());
}
