//! Route middleware trait and types
//!
//! Middleware processes navigation requests before and after navigation occurs.
//! Unlike guards (which decide IF navigation happens), middleware handles
//! cross-cutting concerns like logging, metrics, context setup, etc.
//!
//! # Example
//!
//! ```no_run
//! use gpui_router::{RouteMiddleware, NavigationRequest};
//! use std::future::Future;
//! use std::pin::Pin;
//!
//! struct LoggingMiddleware;
//!
//! impl RouteMiddleware for LoggingMiddleware {
//!     type Future = Pin<Box<dyn Future<Output = ()> + Send>>;
//!
//!     fn before_navigation(&self, cx: &gpui::App, request: &NavigationRequest) -> Self::Future {
//!         println!("Navigating to: {}", request.to);
//!         Box::pin(async {})
//!     }
//!
//!     fn after_navigation(&self, cx: &gpui::App, request: &NavigationRequest) -> Self::Future {
//!         println!("Navigated to: {}", request.to);
//!         Box::pin(async {})
//!     }
//! }
//! ```
use crate::NavigationRequest;
use gpui::App;
use std::future::Future;

/// Middleware that processes navigation requests.
///
/// Middleware runs code before and after navigation, allowing you to:
/// - Log navigation events
/// - Track analytics
/// - Set up context in App
/// - Handle cleanup
/// - Measure performance
///
/// This trait uses associated Future types for zero-cost abstractions
/// and efficient composition.
///
/// # Example
///
/// ```ignore
/// use gpui_router::{RouteMiddleware, NavigationRequest};
/// use std::future::Future;
/// use std::pin::Pin;
///
/// struct AnalyticsMiddleware;
///
/// impl RouteMiddleware for AnalyticsMiddleware {
///     type Future = Pin<Box<dyn Future<Output = ()> + Send>>;
///
///     fn before_navigation(&self, cx: &gpui::App, request: &NavigationRequest) -> Self::Future {
///         let path = request.to.clone();
///         Box::pin(async move {
///             // Track page view
///             println!("Tracking page view: {}", path);
///         })
///     }
///
///     fn after_navigation(&self, cx: &gpui::App, request: &NavigationRequest) -> Self::Future {
///         Box::pin(async {})
///     }
/// }
/// ```
pub trait RouteMiddleware: Send + Sync + 'static {
    /// The future returned by middleware methods
    type Future: Future<Output = ()> + Send + 'static;

    /// Called before navigation occurs
    ///
    /// Use this to:
    /// - Log the navigation attempt
    /// - Set up context in App
    /// - Start timers for performance tracking
    /// - Validate preconditions
    ///
    /// # Example
    ///
    /// ```no_run,ignore
    /// fn before_navigation(&self, cx: &App, request: &NavigationRequest) -> Self::Future {
    ///     println!("Navigating to: {}", request.to);
    ///     Box::pin(async {})
    /// }
    /// ```
    fn before_navigation(&self, cx: &App, request: &NavigationRequest) -> Self::Future;

    /// Called after navigation completes successfully
    ///
    /// Use this to:
    /// - Log successful navigation
    /// - Track analytics
    /// - Clean up resources
    /// - Calculate metrics
    ///
    /// # Example
    ///
    /// ```no_run,ignore
    /// fn after_navigation(&self, cx: &App, request: &NavigationRequest) -> Self::Future {
    ///     println!("Successfully navigated to: {}", request.to);
    ///     Box::pin(async {})
    /// }
    /// ```
    fn after_navigation(&self, cx: &App, request: &NavigationRequest) -> Self::Future;

    /// Middleware name for debugging
    fn name(&self) -> &str {
        "RouteMiddleware"
    }

    /// Middleware priority (higher runs first)
    ///
    /// Default is 0. Use this to control execution order when multiple
    /// middleware are composed.
    fn priority(&self) -> i32 {
        0
    }
}

/// Helper to create middleware from functions
///
/// # Example
///
/// ```ignore
/// use gpui_router::middleware_fn;
///
/// let middleware = middleware_fn(
///     |_cx, request| {
///         println!("Before: {}", request.to);
///         Box::pin(async {})
///     },
///     |_cx, request| {
///         println!("After: {}", request.to);
///         Box::pin(async {})
///     },
/// );
/// ```
pub fn middleware_fn<F, Fut>(before: F, after: F) -> FnMiddleware<F>
where
    F: Fn(&App, &NavigationRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    FnMiddleware { before, after }
}

/// Middleware created from functions
pub struct FnMiddleware<F> {
    before: F,
    after: F,
}

impl<F, Fut> RouteMiddleware for FnMiddleware<F>
where
    F: Fn(&App, &NavigationRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    type Future = Fut;

    fn before_navigation(&self, cx: &App, request: &NavigationRequest) -> Self::Future {
        (self.before)(cx, request)
    }

    fn after_navigation(&self, cx: &App, request: &NavigationRequest) -> Self::Future {
        (self.after)(cx, request)
    }
}

/// Type-erased middleware for dynamic dispatch
pub type BoxedMiddleware =
    Box<dyn RouteMiddleware<Future = std::pin::Pin<Box<dyn Future<Output = ()> + Send>>>>;

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    struct TestMiddleware {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl RouteMiddleware for TestMiddleware {
        type Future = Pin<Box<dyn Future<Output = ()> + Send>>;

        fn before_navigation(&self, _cx: &App, request: &NavigationRequest) -> Self::Future {
            self.calls
                .lock()
                .unwrap()
                .push(format!("before:{}", request.to));
            Box::pin(async {})
        }

        fn after_navigation(&self, _cx: &App, request: &NavigationRequest) -> Self::Future {
            self.calls
                .lock()
                .unwrap()
                .push(format!("after:{}", request.to));
            Box::pin(async {})
        }
    }

    #[gpui::test]
    async fn test_middleware_before(cx: &mut TestAppContext) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let middleware = TestMiddleware {
            calls: calls.clone(),
        };
        let request = NavigationRequest::new("/test".to_string());

        cx.update(|cx| pollster::block_on(middleware.before_navigation(cx, &request)));

        let log = calls.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0], "before:/test");
    }

    #[gpui::test]
    async fn test_middleware_after(cx: &mut TestAppContext) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let middleware = TestMiddleware {
            calls: calls.clone(),
        };
        let request = NavigationRequest::new("/test".to_string());

        cx.update(|cx| pollster::block_on(middleware.after_navigation(cx, &request)));

        let log = calls.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0], "after:/test");
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
}
