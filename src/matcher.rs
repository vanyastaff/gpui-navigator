//! Path pattern parsing and segment matching shared by every resolver.
//!
//! All route matching in the crate (the match stack, [`Route::matches`](crate::Route::matches),
//! named outlets, [`RouteParams::from_path`](crate::RouteParams::from_path)) goes through
//! [`match_prefix`], so patterns behave the same everywhere.
//!
//! # Pattern syntax
//!
//! | Segment | Matches | Captures |
//! |---|---|---|
//! | `users` | the literal segment `users` | nothing |
//! | `:id` | any single segment | `id` |
//! | `:id<int>` (or `:id{int}`) | a single segment that satisfies the constraint | `id` |
//! | `*` | the rest of the path (zero or more segments) | `*` |
//! | `*rest` | the rest of the path (zero or more segments) | `rest` |
//!
//! A wildcard must be the last segment of a pattern.
//!
//! Supported constraints: `int` (alias `i64`), `i32`, `uint` (alias `u64`), `u32`, `usize`,
//! `float` (alias `f64`), `bool`, `uuid`, `alpha`, `alnum` and `slug`.
//!
//! The query string (`?…`) and fragment (`#…`) are never part of matching.

use crate::RouteParams;

/// Parameter name used for the captured rest of an unnamed `*` wildcard.
pub const WILDCARD_PARAM: &str = "*";

/// A type constraint on a `:param<constraint>` segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParamConstraint {
    Int,
    I32,
    Uint,
    U32,
    Usize,
    Float,
    Bool,
    Uuid,
    Alpha,
    Alnum,
    Slug,
}

impl ParamConstraint {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "int" | "i64" => Self::Int,
            "i32" => Self::I32,
            "uint" | "u64" => Self::Uint,
            "u32" => Self::U32,
            "usize" => Self::Usize,
            "float" | "f64" => Self::Float,
            "bool" => Self::Bool,
            "uuid" => Self::Uuid,
            "alpha" => Self::Alpha,
            "alnum" => Self::Alnum,
            "slug" => Self::Slug,
            _ => return None,
        })
    }

    fn accepts(self, value: &str) -> bool {
        match self {
            Self::Int => value.parse::<i64>().is_ok(),
            Self::I32 => value.parse::<i32>().is_ok(),
            Self::Uint => value.parse::<u64>().is_ok(),
            Self::U32 => value.parse::<u32>().is_ok(),
            Self::Usize => value.parse::<usize>().is_ok(),
            Self::Float => value.parse::<f64>().is_ok(),
            Self::Bool => value.parse::<bool>().is_ok(),
            Self::Uuid => is_uuid(value),
            Self::Alpha => !value.is_empty() && value.chars().all(char::is_alphabetic),
            Self::Alnum => !value.is_empty() && value.chars().all(char::is_alphanumeric),
            Self::Slug => {
                !value.is_empty()
                    && value
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            }
        }
    }
}

fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// One parsed segment of a route pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatternSegment<'a> {
    Static(&'a str),
    Param {
        name: &'a str,
        constraint: Option<ParamConstraint>,
    },
    Wildcard {
        name: &'a str,
    },
}

impl<'a> PatternSegment<'a> {
    /// Parse a single pattern segment, reporting syntax errors.
    pub(crate) fn parse(segment: &'a str) -> Result<Self, String> {
        if let Some(rest) = segment.strip_prefix(':') {
            let (name, constraint) = match rest.find(['<', '{']) {
                Some(pos) => {
                    let close = if rest.as_bytes()[pos] == b'<' {
                        '>'
                    } else {
                        '}'
                    };
                    let Some(inner) = rest[pos + 1..].strip_suffix(close) else {
                        return Err(format!(
                            "Route parameter '{rest}' has an unterminated constraint (expected ':name<type>')"
                        ));
                    };
                    let Some(constraint) = ParamConstraint::parse(inner) else {
                        return Err(format!(
                            "Unknown constraint '{inner}' on route parameter '{}'",
                            &rest[..pos]
                        ));
                    };
                    (&rest[..pos], Some(constraint))
                }
                None => (rest, None),
            };
            if name.is_empty() {
                return Err("Route parameter name cannot be empty".to_string());
            }
            if !is_param_name(name) {
                return Err(format!(
                    "Route parameter '{name}' must contain only alphanumeric characters and underscores"
                ));
            }
            return Ok(Self::Param { name, constraint });
        }

        if let Some(name) = segment.strip_prefix('*') {
            if !name.is_empty() && !is_param_name(name) {
                return Err(format!(
                    "Wildcard name '{name}' must contain only alphanumeric characters and underscores"
                ));
            }
            let name = if name.is_empty() {
                WILDCARD_PARAM
            } else {
                name
            };
            return Ok(Self::Wildcard { name });
        }

        Ok(Self::Static(segment))
    }

    /// Parse leniently: a malformed segment is treated as a literal.
    fn parse_lenient(segment: &'a str) -> Self {
        Self::parse(segment).unwrap_or(Self::Static(segment))
    }
}

fn is_param_name(name: &str) -> bool {
    name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Split a location into its path and query parts, dropping any `#fragment`.
///
/// ```text
/// "/users?tab=1#top" → ("/users", "tab=1")
/// "/users"           → ("/users", "")
/// ```
#[must_use]
pub fn split_path_and_query(location: &str) -> (&str, &str) {
    let without_fragment = location.split_once('#').map_or(location, |(p, _)| p);
    without_fragment
        .split_once('?')
        .unwrap_or((without_fragment, ""))
}

/// Strip the query string and fragment from a location.
#[must_use]
pub fn path_only(location: &str) -> &str {
    split_path_and_query(location).0
}

/// Split a location into its non-empty path segments (query and fragment excluded).
#[must_use]
pub(crate) fn segments(location: &str) -> Vec<&str> {
    path_only(location)
        .split('/')
        .filter(|s| !s.is_empty())
        .collect()
}

/// Split a pattern into its non-empty segments.
pub(crate) fn pattern_segments(pattern: &str) -> impl Iterator<Item = &str> {
    pattern.split('/').filter(|s| !s.is_empty())
}

/// Validate every segment of a pattern (syntax, constraints, wildcard placement,
/// duplicate parameter names).
pub(crate) fn validate_pattern(pattern: &str) -> Result<(), String> {
    let mut names = std::collections::HashSet::new();
    let segs: Vec<&str> = pattern_segments(pattern).collect();
    for (i, seg) in segs.iter().enumerate() {
        match PatternSegment::parse(seg)? {
            PatternSegment::Static(_) => {}
            PatternSegment::Param { name, .. } => {
                if !names.insert(name) {
                    return Err(format!("Duplicate route parameter: '{name}'"));
                }
            }
            PatternSegment::Wildcard { name } => {
                if i + 1 != segs.len() {
                    return Err(format!(
                        "Wildcard '{seg}' must be the last segment of the route path"
                    ));
                }
                if !names.insert(name) {
                    return Err(format!("Duplicate route parameter: '{name}'"));
                }
            }
        }
    }
    Ok(())
}

/// Return `true` if every path matched by `later` is already matched by
/// `earlier`, so a route registered with `later` after `earlier` (as a
/// sibling) can never be reached.
///
/// Conservative: constrained parameters only shadow identical constraints.
#[must_use]
pub(crate) fn shadows(earlier: &str, later: &str) -> bool {
    let mut earlier_segs = pattern_segments(earlier).map(PatternSegment::parse_lenient);
    let mut later_segs = pattern_segments(later).map(PatternSegment::parse_lenient);
    loop {
        match (earlier_segs.next(), later_segs.next()) {
            (None, None) | (Some(PatternSegment::Wildcard { .. }), _) => return true,
            (Some(PatternSegment::Static(a)), Some(PatternSegment::Static(b))) if a == b => {}
            (
                Some(PatternSegment::Param {
                    constraint: None, ..
                }),
                Some(PatternSegment::Static(_) | PatternSegment::Param { .. }),
            ) => {}
            (
                Some(PatternSegment::Param {
                    constraint: Some(a),
                    ..
                }),
                Some(PatternSegment::Param {
                    constraint: Some(b),
                    ..
                }),
            ) if a == b => {}
            _ => return false,
        }
    }
}

/// Match `pattern` against the start of `remaining`.
///
/// On success, captured parameters are inserted into `params` and the number of
/// consumed segments is returned. A trailing wildcard consumes everything left.
/// On failure `params` is left untouched.
pub(crate) fn match_prefix(
    pattern: &str,
    remaining: &[&str],
    params: &mut RouteParams,
) -> Option<usize> {
    let mut captured: Vec<(&str, String)> = Vec::new();
    let mut consumed = 0;

    for seg in pattern_segments(pattern) {
        match PatternSegment::parse_lenient(seg) {
            PatternSegment::Wildcard { name } => {
                captured.push((name, remaining[consumed..].join("/")));
                consumed = remaining.len();
                break;
            }
            PatternSegment::Static(lit) => {
                if remaining.get(consumed) != Some(&lit) {
                    return None;
                }
                consumed += 1;
            }
            PatternSegment::Param { name, constraint } => {
                let value = remaining.get(consumed)?;
                if constraint.is_some_and(|c| !c.accepts(value)) {
                    return None;
                }
                captured.push((name, (*value).to_string()));
                consumed += 1;
            }
        }
    }

    for (name, value) in captured {
        params.insert(name, value);
    }
    Some(consumed)
}

/// Match `pattern` against the whole of `location` (query string ignored).
#[must_use]
pub(crate) fn match_full(pattern: &str, location: &str) -> Option<RouteParams> {
    let segs = segments(location);
    let mut params = RouteParams::new();
    let consumed = match_prefix(pattern, &segs, &mut params)?;
    (consumed == segs.len()).then_some(params)
}

/// Substitute `:param` and `*wildcard` segments of `pattern` with values from `params`.
///
/// Returns `None` if a parameter is missing or a value violates its constraint.
#[must_use]
pub(crate) fn build_path(pattern: &str, params: &RouteParams) -> Option<String> {
    let mut out = String::new();
    for seg in pattern_segments(pattern) {
        let piece = match PatternSegment::parse_lenient(seg) {
            PatternSegment::Static(lit) => lit.to_string(),
            PatternSegment::Param { name, constraint } => {
                let value = params.get(name)?;
                if constraint.is_some_and(|c| !c.accepts(value)) {
                    return None;
                }
                value.clone()
            }
            PatternSegment::Wildcard { name } => {
                let value = params.get(name).map_or("", String::as_str);
                value.trim_matches('/').to_string()
            }
        };
        if !piece.is_empty() {
            out.push('/');
            out.push_str(&piece);
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pattern: &str, path: &str) -> Option<RouteParams> {
        match_full(pattern, path)
    }

    #[test]
    fn query_and_fragment_are_ignored() {
        assert!(m("/users", "/users?tab=1").is_some());
        assert!(m("/users", "/users#top").is_some());
        assert_eq!(
            m("/users/:id", "/users/7?x=1#y").unwrap().get("id"),
            Some(&"7".to_string())
        );
        assert_eq!(split_path_and_query("/a?b=1#c"), ("/a", "b=1"));
    }

    #[test]
    fn wildcard_captures_rest() {
        let p = m("/files/*", "/files/a/b").unwrap();
        assert_eq!(p.get("*"), Some(&"a/b".to_string()));
        let p = m("/files/*path", "/files/a/b/c.txt").unwrap();
        assert_eq!(p.get("path"), Some(&"a/b/c.txt".to_string()));
        assert!(m("/files/*", "/files").is_some());
        assert!(m("/files/*", "/other").is_none());
        assert!(m("*", "/anything/at/all").is_some());
    }

    #[test]
    fn constraints_filter_values() {
        assert!(m("/n/:id<int>", "/n/42").is_some());
        assert!(m("/n/:id<int>", "/n/abc").is_none());
        assert!(m("/n/:id<u32>", "/n/-1").is_none());
        assert!(m("/n/:flag<bool>", "/n/true").is_some());
        assert!(m("/n/:id<uuid>", "/n/123e4567-e89b-12d3-a456-426614174000").is_some());
        assert!(m("/n/:id<uuid>", "/n/123").is_none());
        assert!(m("/n/:s<slug>", "/n/hello-world_1").is_some());
        assert!(m("/n/:s<alpha>", "/n/abc1").is_none());
    }

    #[test]
    fn validation() {
        assert!(validate_pattern("/a/:id<int>/*rest").is_ok());
        assert!(validate_pattern("/a/:id{uuid}").is_ok());
        assert!(validate_pattern("/a/:id<nope>").is_err());
        assert!(validate_pattern("/a/:id<int").is_err());
        assert!(validate_pattern("/a/*/b").is_err());
        assert!(validate_pattern("/a/:id/:id").is_err());
    }

    #[test]
    fn shadowing() {
        assert!(shadows("/:id", "/about"));
        assert!(shadows("/users/:id", "/users/:name"));
        assert!(shadows("*", "/anything"));
        assert!(shadows("/users", "users/"));
        assert!(!shadows("/about", "/:id"));
        assert!(!shadows("/:id<int>", "/about"));
        assert!(!shadows("/users/:id", "/users/:id/edit"));
        assert!(!shadows("/users/:id", "/users/*rest"));
    }

    #[test]
    fn build_path_substitutes_whole_segments() {
        let mut p = RouteParams::new();
        p.insert("id", "1");
        p.insert("idx", "2");
        assert_eq!(build_path("/a/:id/:idx", &p).as_deref(), Some("/a/1/2"));
        p.insert("rest", "x/y");
        assert_eq!(build_path("/f/*rest", &p).as_deref(), Some("/f/x/y"));
        assert_eq!(build_path("/a/:missing", &p), None);
    }
}
