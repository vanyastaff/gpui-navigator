//! Error handling for the router.
//!
//! This module defines the types returned when a navigation attempt cannot
//! complete successfully:
//!
//! - [`NavigationResult`] — the top-level outcome of any navigation
//!   (`Success`, `NotFound`, `Blocked`, `Error`).
//! - [`NavigationError`] — a detailed error variant (route not found, guard
//!   blocked, invalid params, etc.).
//! - [`ErrorHandlers`] — a builder for registering custom 404 and error page
//!   renderers.
//!
//! # Examples
//!
//! ```
//! use gpui_navigator::error::NavigationResult;
//!
//! let result = NavigationResult::Success { path: "/home".into() };
//! assert!(result.is_success());
//!
//! let blocked = NavigationResult::Blocked {
//!     reason: "Not authenticated".into(),
//!     redirect: Some("/login".into()),
//! };
//! assert_eq!(blocked.redirect_path(), Some("/login"));
//! ```

use gpui::{AnyElement, App};
use std::fmt;
use std::sync::Arc;

// ============================================================================
// Navigation Result Types
// ============================================================================

/// Outcome of a navigation attempt through the guard/middleware pipeline.
///
/// Every call to [`GlobalRouter::push`](crate::context::GlobalRouter::push)
/// (and friends) returns this enum.
#[derive(Debug, Clone)]
pub enum NavigationResult {
    /// Navigation succeeded
    Success { path: String },
    /// Route not found
    NotFound { path: String },
    /// Navigation blocked by guard
    Blocked {
        reason: String,
        redirect: Option<String>,
    },
    /// Navigation error
    Error(NavigationError),
}

/// Detailed error variants that can occur during navigation.
///
/// Implements [`std::error::Error`] and [`Display`](std::fmt::Display) for
/// idiomatic error handling.
#[derive(Debug, Clone)]
pub enum NavigationError {
    /// Route not found
    RouteNotFound { path: String },

    /// Guard blocked navigation
    GuardBlocked { reason: String },

    /// Invalid route parameters
    InvalidParams { message: String },

    /// Navigation failed
    NavigationFailed { message: String },

    /// Custom error
    Custom { message: String },
}

impl fmt::Display for NavigationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NavigationError::RouteNotFound { path } => {
                write!(f, "Route not found: {}", path)
            }
            NavigationError::GuardBlocked { reason } => {
                write!(f, "Navigation blocked: {}", reason)
            }
            NavigationError::InvalidParams { message } => {
                write!(f, "Invalid parameters: {}", message)
            }
            NavigationError::NavigationFailed { message } => {
                write!(f, "Navigation failed: {}", message)
            }
            NavigationError::Custom { message } => {
                write!(f, "{}", message)
            }
        }
    }
}

impl std::error::Error for NavigationError {}

impl NavigationResult {
    /// Check if navigation was successful
    pub fn is_success(&self) -> bool {
        matches!(self, NavigationResult::Success { .. })
    }

    /// Check if route was not found
    pub fn is_not_found(&self) -> bool {
        matches!(self, NavigationResult::NotFound { .. })
    }

    /// Check if navigation was blocked
    pub fn is_blocked(&self) -> bool {
        matches!(self, NavigationResult::Blocked { .. })
    }

    /// Check if there was an error
    pub fn is_error(&self) -> bool {
        matches!(self, NavigationResult::Error(_))
    }

    /// Get redirect path if blocked with redirect
    pub fn redirect_path(&self) -> Option<&str> {
        match self {
            NavigationResult::Blocked {
                redirect: Some(path),
                ..
            } => Some(path),
            _ => None,
        }
    }
}

// ============================================================================
// Error Handlers
// ============================================================================

/// Handler for navigation errors
pub type ErrorHandler = Arc<dyn Fn(&mut App, &NavigationError) -> AnyElement + Send + Sync>;

/// Handler for 404 not found
pub type NotFoundHandler = Arc<dyn Fn(&mut App, &str) -> AnyElement + Send + Sync>;

/// Builder for registering custom error-page renderers.
///
/// # Examples
///
/// ```ignore
/// use gpui_navigator::error::ErrorHandlers;
///
/// let handlers = ErrorHandlers::new()
///     .on_not_found(|cx, path| {
///         gpui::div().child(format!("404: {path}")).into_any_element()
///     })
///     .on_error(|cx, err| {
///         gpui::div().child(format!("Error: {err}")).into_any_element()
///     });
/// ```
pub struct ErrorHandlers {
    /// Handler for 404 not found errors
    pub not_found: Option<NotFoundHandler>,

    /// Handler for general navigation errors
    pub error: Option<ErrorHandler>,
}

impl ErrorHandlers {
    /// Create new empty error handlers
    pub fn new() -> Self {
        Self {
            not_found: None,
            error: None,
        }
    }

    /// Set the 404 not found handler
    pub fn on_not_found<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut App, &str) -> AnyElement + Send + Sync + 'static,
    {
        self.not_found = Some(Arc::new(handler));
        self
    }

    /// Set the general error handler
    pub fn on_error<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut App, &NavigationError) -> AnyElement + Send + Sync + 'static,
    {
        self.error = Some(Arc::new(handler));
        self
    }

    /// Render a 404 not found page
    pub fn render_not_found(&self, cx: &mut App, path: &str) -> Option<AnyElement> {
        self.not_found.as_ref().map(|handler| handler(cx, path))
    }

    /// Render an error page
    pub fn render_error(&self, cx: &mut App, error: &NavigationError) -> Option<AnyElement> {
        self.error.as_ref().map(|handler| handler(cx, error))
    }
}

impl Default for ErrorHandlers {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{div, IntoElement, ParentElement, TestAppContext};

    #[test]
    fn test_navigation_result_success() {
        let result = NavigationResult::Success {
            path: "/home".to_string(),
        };
        assert!(result.is_success());
        assert!(!result.is_not_found());
        assert!(!result.is_blocked());
        assert!(!result.is_error());
    }

    #[test]
    fn test_navigation_result_not_found() {
        let result = NavigationResult::NotFound {
            path: "/invalid".to_string(),
        };
        assert!(!result.is_success());
        assert!(result.is_not_found());
    }

    #[test]
    fn test_navigation_result_blocked_with_redirect() {
        let result = NavigationResult::Blocked {
            reason: "Not authenticated".to_string(),
            redirect: Some("/login".to_string()),
        };
        assert!(result.is_blocked());
        assert_eq!(result.redirect_path(), Some("/login"));
    }

    #[test]
    fn test_navigation_error_display() {
        let error = NavigationError::RouteNotFound {
            path: "/test".to_string(),
        };
        assert_eq!(error.to_string(), "Route not found: /test");
    }

    #[test]
    fn test_error_handlers_creation() {
        let handlers = ErrorHandlers::new();
        assert!(handlers.not_found.is_none());
        assert!(handlers.error.is_none());
    }

    #[gpui::test]
    async fn test_on_not_found(cx: &mut TestAppContext) {
        let handlers = ErrorHandlers::new()
            .on_not_found(|_cx, path| div().child(format!("404: {}", path)).into_any_element());

        assert!(handlers.not_found.is_some());

        let element = cx.update(|cx| handlers.render_not_found(cx, "/invalid"));
        assert!(element.is_some());
    }

    #[gpui::test]
    async fn test_on_error(cx: &mut TestAppContext) {
        let handlers = ErrorHandlers::new()
            .on_error(|_cx, error| div().child(format!("Error: {}", error)).into_any_element());

        assert!(handlers.error.is_some());

        let error = NavigationError::RouteNotFound {
            path: "/test".to_string(),
        };

        let element = cx.update(|cx| handlers.render_error(cx, &error));
        assert!(element.is_some());
    }
}
