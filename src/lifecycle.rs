//! Route lifecycle hooks and navigation action types.
//!
//! This module defines two key abstractions:
//!
//! - [`NavigationAction`] — the unified result type returned by guards, lifecycle
//!   hooks, and middleware. It describes whether navigation should continue, be
//!   denied, or be redirected.
//! - [`RouteLifecycle`] — a trait for running code at key points in the navigation
//!   process: entering a route, exiting a route, and checking whether the user
//!   can leave (e.g. unsaved changes prompt).
//!
//! # Navigation pipeline
//!
//! When a navigation request is made, the router executes steps in this order:
//!
//! 1. **Guards** — decide if navigation is allowed (see [`guards`](crate::guards))
//! 2. **`can_deactivate`** — current route's lifecycle check
//! 3. **Middleware `before`** — cross-cutting pre-navigation logic
//! 4. **`on_exit`** — current route's cleanup
//! 5. **Navigation** — the route change itself
//! 6. **`on_enter`** — new route's setup
//! 7. **Middleware `after`** — cross-cutting post-navigation logic

use crate::NavigationRequest;
use gpui::App;

// ============================================================================
// NavigationAction — unified result for guards, lifecycle, middleware
// ============================================================================

/// Result of a navigation check (guard, lifecycle, or middleware).
///
/// Used by guards to allow/deny navigation, by lifecycle hooks to
/// continue/abort, and as a general "what should the router do?" answer.
///
/// # Example
///
/// ```
/// use gpui_navigator::NavigationAction;
///
/// let action = NavigationAction::deny("Not authorized");
/// assert!(action.is_deny());
///
/// let action = NavigationAction::redirect("/login");
/// assert_eq!(action.redirect_path(), Some("/login"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationAction {
    /// Allow navigation to proceed.
    Continue,

    /// Deny navigation with a reason.
    Deny {
        /// Human-readable reason for denying navigation.
        reason: String,
    },

    /// Redirect to a different path.
    Redirect {
        /// Path to redirect to.
        to: String,
        /// Optional human-readable reason for redirecting.
        reason: Option<String>,
    },
}

impl NavigationAction {
    /// Create a result that allows navigation to proceed (alias for [`Continue`](Self::Continue)).
    pub fn allow() -> Self {
        Self::Continue
    }

    /// Create a result that blocks navigation with a human-readable reason.
    pub fn deny(reason: impl Into<String>) -> Self {
        Self::Deny {
            reason: reason.into(),
        }
    }

    /// Create a result that redirects navigation to a different path.
    pub fn redirect(to: impl Into<String>) -> Self {
        Self::Redirect {
            to: to.into(),
            reason: None,
        }
    }

    /// Create a redirect result with a human-readable reason.
    pub fn redirect_with_reason(to: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Redirect {
            to: to.into(),
            reason: Some(reason.into()),
        }
    }

    /// Check if this action allows navigation to continue.
    pub fn is_continue(&self) -> bool {
        matches!(self, Self::Continue)
    }

    /// Check if this action denies navigation.
    pub fn is_deny(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }

    /// Check if this action redirects navigation.
    pub fn is_redirect(&self) -> bool {
        matches!(self, Self::Redirect { .. })
    }

    /// Get the redirect path, if this is a redirect action.
    pub fn redirect_path(&self) -> Option<&str> {
        match self {
            Self::Redirect { to, .. } => Some(to.as_str()),
            _ => None,
        }
    }
}

// Backward-compatibility aliases

/// Deprecated alias for [`NavigationAction`].
#[deprecated(since = "0.2.0", note = "Use NavigationAction instead")]
pub type GuardResult = NavigationAction;

/// Deprecated alias for [`NavigationAction`].
#[deprecated(since = "0.2.0", note = "Use NavigationAction instead")]
pub type LifecycleResult = NavigationAction;

// ============================================================================
// RouteLifecycle trait
// ============================================================================

/// Route lifecycle hooks.
///
/// Lifecycle hooks allow you to run code at key points in the navigation process:
/// - [`on_enter`](RouteLifecycle::on_enter): Called when entering a route (for data loading, setup)
/// - [`on_exit`](RouteLifecycle::on_exit): Called when leaving a route (for cleanup, saving state)
/// - [`can_deactivate`](RouteLifecycle::can_deactivate): Called to check if user can leave (for unsaved changes warning)
///
/// All methods are **synchronous** because GPUI is a single-threaded desktop framework.
///
/// # Example
///
/// ```no_run
/// use gpui_navigator::{RouteLifecycle, NavigationAction, NavigationRequest};
///
/// struct FormLifecycle {
///     has_unsaved_changes: bool,
/// }
///
/// impl RouteLifecycle for FormLifecycle {
///     fn on_enter(&self, _cx: &gpui::App, _request: &NavigationRequest) -> NavigationAction {
///         NavigationAction::Continue
///     }
///
///     fn on_exit(&self, _cx: &gpui::App) -> NavigationAction {
///         NavigationAction::Continue
///     }
///
///     fn can_deactivate(&self, _cx: &gpui::App) -> NavigationAction {
///         if self.has_unsaved_changes {
///             NavigationAction::deny("Unsaved changes")
///         } else {
///             NavigationAction::Continue
///         }
///     }
/// }
/// ```
pub trait RouteLifecycle: Send + Sync + 'static {
    /// Called when entering the route.
    ///
    /// Use this to load data, set up subscriptions, or validate navigation parameters.
    /// Return [`NavigationAction::deny`] to prevent navigation
    /// or [`NavigationAction::redirect`] to navigate elsewhere.
    fn on_enter(&self, cx: &App, request: &NavigationRequest) -> NavigationAction;

    /// Called when exiting the route.
    ///
    /// Use this to save state, clean up subscriptions, or cancel pending operations.
    /// Return [`NavigationAction::deny`] to prevent navigation away.
    fn on_exit(&self, cx: &App) -> NavigationAction;

    /// Check if the route can be deactivated.
    ///
    /// Use this to check for unsaved changes or confirm navigation away.
    /// Return [`NavigationAction::deny`] to prevent navigation.
    fn can_deactivate(&self, cx: &App) -> NavigationAction;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- NavigationAction tests ---

    #[test]
    fn test_navigation_action_continue() {
        let action = NavigationAction::Continue;
        assert!(action.is_continue());
        assert!(!action.is_deny());
        assert!(!action.is_redirect());
        assert_eq!(action.redirect_path(), None);
    }

    #[test]
    fn test_navigation_action_allow_alias() {
        let action = NavigationAction::allow();
        assert!(action.is_continue());
    }

    #[test]
    fn test_navigation_action_deny() {
        let action = NavigationAction::deny("Not authorized");
        assert!(!action.is_continue());
        assert!(action.is_deny());
        assert!(!action.is_redirect());

        match action {
            NavigationAction::Deny { reason } => assert_eq!(reason, "Not authorized"),
            _ => panic!("Expected Deny"),
        }
    }

    #[test]
    fn test_navigation_action_redirect() {
        let action = NavigationAction::redirect("/login");
        assert!(!action.is_continue());
        assert!(!action.is_deny());
        assert!(action.is_redirect());
        assert_eq!(action.redirect_path(), Some("/login"));
    }

    #[test]
    fn test_navigation_action_redirect_with_reason() {
        let action = NavigationAction::redirect_with_reason("/login", "Auth required");
        match action {
            NavigationAction::Redirect { to, reason } => {
                assert_eq!(to, "/login");
                assert_eq!(reason, Some("Auth required".to_string()));
            }
            _ => panic!("Expected Redirect"),
        }
    }

    #[test]
    fn test_navigation_action_equality() {
        assert_eq!(NavigationAction::Continue, NavigationAction::Continue);
        assert_ne!(NavigationAction::Continue, NavigationAction::deny("x"));
    }

    // --- RouteLifecycle tests ---

    struct TestLifecycle {
        should_abort: bool,
        should_redirect: bool,
    }

    impl RouteLifecycle for TestLifecycle {
        fn on_enter(&self, _cx: &App, _request: &NavigationRequest) -> NavigationAction {
            if self.should_abort {
                NavigationAction::deny("Test abort")
            } else if self.should_redirect {
                NavigationAction::redirect("/redirect")
            } else {
                NavigationAction::Continue
            }
        }

        fn on_exit(&self, _cx: &App) -> NavigationAction {
            NavigationAction::Continue
        }

        fn can_deactivate(&self, _cx: &App) -> NavigationAction {
            if self.should_abort {
                NavigationAction::deny("Cannot leave")
            } else {
                NavigationAction::Continue
            }
        }
    }

    #[gpui::test]
    fn test_lifecycle_on_enter_continue(cx: &mut gpui::TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: false,
            should_redirect: false,
        };
        let request = NavigationRequest::new("/test".to_string());
        let result = cx.update(|cx| lifecycle.on_enter(cx, &request));
        assert_eq!(result, NavigationAction::Continue);
    }

    #[gpui::test]
    fn test_lifecycle_on_enter_abort(cx: &mut gpui::TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: true,
            should_redirect: false,
        };
        let request = NavigationRequest::new("/test".to_string());
        let result = cx.update(|cx| lifecycle.on_enter(cx, &request));
        assert!(result.is_deny());
    }

    #[gpui::test]
    fn test_lifecycle_on_enter_redirect(cx: &mut gpui::TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: false,
            should_redirect: true,
        };
        let request = NavigationRequest::new("/test".to_string());
        let result = cx.update(|cx| lifecycle.on_enter(cx, &request));
        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/redirect"));
    }

    #[gpui::test]
    fn test_lifecycle_can_deactivate_allow(cx: &mut gpui::TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: false,
            should_redirect: false,
        };
        let result = cx.update(|cx| lifecycle.can_deactivate(cx));
        assert_eq!(result, NavigationAction::Continue);
    }

    #[gpui::test]
    fn test_lifecycle_can_deactivate_block(cx: &mut gpui::TestAppContext) {
        let lifecycle = TestLifecycle {
            should_abort: true,
            should_redirect: false,
        };
        let result = cx.update(|cx| lifecycle.can_deactivate(cx));
        assert!(result.is_deny());
    }
}
