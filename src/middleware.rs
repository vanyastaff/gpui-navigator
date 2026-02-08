//! Route middleware for cross-cutting navigation concerns.
//!
//! Middleware runs **before** and **after** navigation. Unlike guards (which
//! decide *if* navigation happens), middleware handles side effects like
//! logging, metrics, context setup, and cleanup.
//!
//! All methods are **synchronous** — GPUI is a single-threaded desktop framework.
//!
//! # Execution order
//!
//! When multiple middleware are attached, they execute in **priority order**
//! (higher [`priority`](RouteMiddleware::priority) first) for `before_navigation`,
//! and in reverse order for `after_navigation` (onion model).
//!
//! # Creating middleware
//!
//! | Approach | When to use |
//! |----------|-------------|
//! | Implement [`RouteMiddleware`] | Full control, named, with custom priority |
//! | [`middleware_fn`] | Quick one-off from two closures |
//!
//! # Example
//!
//! ```no_run
//! use gpui_navigator::{RouteMiddleware, NavigationRequest};
//!
//! struct LoggingMiddleware;
//!
//! impl RouteMiddleware for LoggingMiddleware {
//!     fn before_navigation(&self, _cx: &gpui::App, request: &NavigationRequest) {
//!         println!("Navigating to: {}", request.to);
//!     }
//!
//!     fn after_navigation(&self, _cx: &gpui::App, request: &NavigationRequest) {
//!         println!("Navigated to: {}", request.to);
//!     }
//! }
//! ```

use crate::NavigationRequest;
use gpui::App;

// ============================================================================
// RouteMiddleware trait
// ============================================================================

/// Middleware that processes navigation requests.
///
/// Middleware runs code before and after navigation, allowing you to:
/// - Log navigation events
/// - Track analytics
/// - Set up context
/// - Handle cleanup
/// - Measure performance
///
/// # Example
///
/// ```no_run
/// use gpui_navigator::{RouteMiddleware, NavigationRequest};
///
/// struct AnalyticsMiddleware;
///
/// impl RouteMiddleware for AnalyticsMiddleware {
///     fn before_navigation(&self, _cx: &gpui::App, request: &NavigationRequest) {
///         println!("Tracking page view: {}", request.to);
///     }
///
///     fn after_navigation(&self, _cx: &gpui::App, _request: &NavigationRequest) {
///         // Cleanup or finalize tracking
///     }
/// }
/// ```
pub trait RouteMiddleware: Send + Sync + 'static {
    /// Called before navigation occurs.
    fn before_navigation(&self, cx: &App, request: &NavigationRequest);

    /// Called after navigation completes successfully.
    fn after_navigation(&self, cx: &App, request: &NavigationRequest);

    /// Middleware name for debugging.
    fn name(&self) -> &'static str {
        "RouteMiddleware"
    }

    /// Middleware priority (higher runs first for `before`, last for `after`).
    fn priority(&self) -> i32 {
        0
    }
}

// ============================================================================
// middleware_fn helper
// ============================================================================

/// Create middleware from two closures (before and after).
///
/// Unlike the previous design, the two closures can be **different types**.
///
/// # Example
///
/// ```no_run
/// use gpui_navigator::middleware_fn;
///
/// let mw = middleware_fn(
///     |_cx, request| {
///         println!("Before: {}", request.to);
///     },
///     |_cx, request| {
///         println!("After: {}", request.to);
///     },
/// );
/// ```
pub const fn middleware_fn<B, A>(before: B, after: A) -> FnMiddleware<B, A>
where
    B: Fn(&App, &NavigationRequest) + Send + Sync + 'static,
    A: Fn(&App, &NavigationRequest) + Send + Sync + 'static,
{
    FnMiddleware { before, after }
}

/// Middleware created from two closures via [`middleware_fn`].
pub struct FnMiddleware<B, A> {
    before: B,
    after: A,
}

impl<B, A> RouteMiddleware for FnMiddleware<B, A>
where
    B: Fn(&App, &NavigationRequest) + Send + Sync + 'static,
    A: Fn(&App, &NavigationRequest) + Send + Sync + 'static,
{
    fn before_navigation(&self, cx: &App, request: &NavigationRequest) {
        (self.before)(cx, request);
    }

    fn after_navigation(&self, cx: &App, request: &NavigationRequest) {
        (self.after)(cx, request);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::needless_pass_by_ref_mut)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use std::sync::{Arc, Mutex};

    struct TestMiddleware {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl RouteMiddleware for TestMiddleware {
        fn before_navigation(&self, _cx: &App, request: &NavigationRequest) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("before:{}", request.to));
        }

        fn after_navigation(&self, _cx: &App, request: &NavigationRequest) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("after:{}", request.to));
        }
    }

    #[gpui::test]
    fn test_middleware_before(cx: &mut TestAppContext) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let middleware = TestMiddleware {
            calls: calls.clone(),
        };
        let request = NavigationRequest::new("/test".to_string());

        cx.update(|cx| middleware.before_navigation(cx, &request));

        let log = calls.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0], "before:/test");
        drop(log);
    }

    #[gpui::test]
    fn test_middleware_after(cx: &mut TestAppContext) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let middleware = TestMiddleware {
            calls: calls.clone(),
        };
        let request = NavigationRequest::new("/test".to_string());

        cx.update(|cx| middleware.after_navigation(cx, &request));

        let log = calls.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0], "after:/test");
        drop(log);
    }

    #[test]
    fn test_middleware_name() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let middleware = TestMiddleware { calls };
        assert_eq!(middleware.name(), "RouteMiddleware");
    }

    #[test]
    fn test_middleware_priority() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let middleware = TestMiddleware { calls };
        assert_eq!(middleware.priority(), 0);
    }

    #[gpui::test]
    fn test_middleware_fn_different_closures(cx: &mut TestAppContext) {
        let before_calls = Arc::new(Mutex::new(Vec::new()));
        let after_calls = Arc::new(Mutex::new(Vec::new()));
        let before_clone = before_calls.clone();
        let after_clone = after_calls.clone();

        let mw = middleware_fn(
            move |_cx, req| {
                before_clone.lock().unwrap().push(format!("B:{}", req.to));
            },
            move |_cx, req| {
                after_clone.lock().unwrap().push(format!("A:{}", req.to));
            },
        );

        let request = NavigationRequest::new("/page".to_string());
        cx.update(|cx| {
            mw.before_navigation(cx, &request);
            mw.after_navigation(cx, &request);
        });

        assert_eq!(*before_calls.lock().unwrap(), vec!["B:/page"]);
        assert_eq!(*after_calls.lock().unwrap(), vec!["A:/page"]);
    }
}
