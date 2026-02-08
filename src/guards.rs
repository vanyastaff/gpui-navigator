//! Route guards for authentication, authorization, and validation.
//!
//! Guards are checked **before** navigation proceeds. They decide whether a
//! navigation should be allowed, denied, or redirected elsewhere.
//!
//! All guard methods are **synchronous** --- GPUI is a single-threaded desktop
//! framework and there is no need for async guard checks.
//!
//! # Built-in guards
//!
//! | Guard | Purpose |
//! |-------|---------|
//! | [`AuthGuard`] | Checks authentication via a user-provided function |
//! | [`RoleGuard`] | Checks role-based authorization |
//! | [`PermissionGuard`] | Checks specific permissions |
//!
//! # Composition
//!
//! | Combinator | Logic |
//! |------------|-------|
//! | [`Guards`] | AND — all guards must allow |
//! | [`NotGuard`] | Invert — allow becomes deny, deny becomes allow |
//!
//! # Execution order
//!
//! Guards run in **priority order** (higher value first). The built-in guards
//! use: `AuthGuard` = 100, `RoleGuard` = 90, `PermissionGuard` = 80.
//! The first non-[`Continue`](crate::NavigationAction::Continue) result
//! short-circuits evaluation.
//!
//! # Example
//!
//! ```no_run
//! use gpui::IntoElement;
//! use gpui_navigator::{Route, AuthGuard, RoleGuard, NavigationAction};
//!
//! Route::new("/admin", |_, _cx, _params| gpui::div().into_any_element())
//!     .guard(AuthGuard::new(|_cx| true, "/login"))
//!     .guard(RoleGuard::new(|_cx| Some("admin".into()), "admin", Some("/forbidden")));
//! ```

use crate::lifecycle::NavigationAction;
use crate::NavigationRequest;
use gpui::App;

// ============================================================================
// RouteGuard trait
// ============================================================================

/// Trait for route guards that control access to routes.
///
/// Guards are checked synchronously before navigation proceeds.
///
/// # Example
///
/// ```no_run
/// use gpui_navigator::{RouteGuard, NavigationAction, NavigationRequest};
///
/// struct MyAuthGuard {
///     redirect_to: String,
/// }
///
/// impl RouteGuard for MyAuthGuard {
///     fn check(&self, _cx: &gpui::App, _request: &NavigationRequest) -> NavigationAction {
///         let is_authenticated = true; // Replace with actual check
///         if is_authenticated {
///             NavigationAction::Continue
///         } else {
///             NavigationAction::redirect(&self.redirect_to)
///         }
///     }
/// }
/// ```
///
/// # For simple guards
///
/// Use [`guard_fn`] to create a guard from a closure:
///
/// ```no_run
/// use gpui_navigator::{guard_fn, NavigationAction};
///
/// let guard = guard_fn(|_cx, _request| {
///     NavigationAction::Continue
/// });
/// ```
pub trait RouteGuard: Send + Sync + 'static {
    /// Check if navigation should be allowed.
    ///
    /// Returns:
    /// - [`NavigationAction::Continue`] to allow navigation
    /// - [`NavigationAction::Deny`] to block navigation
    /// - [`NavigationAction::Redirect`] to redirect to a different path
    fn check(&self, cx: &App, request: &NavigationRequest) -> NavigationAction;

    /// Guard name for debugging and error messages.
    fn name(&self) -> &'static str {
        "RouteGuard"
    }

    /// Priority for execution order. Higher runs first. Default is 0.
    fn priority(&self) -> i32 {
        0
    }
}

// ============================================================================
// guard_fn helper
// ============================================================================

/// Create a guard from a function or closure.
///
/// # Example
///
/// ```no_run
/// use gpui_navigator::{guard_fn, NavigationAction};
///
/// let auth_guard = guard_fn(|_cx, _request| {
///     let is_authenticated = true; // Replace with actual check
///     if is_authenticated {
///         NavigationAction::Continue
///     } else {
///         NavigationAction::redirect("/login")
///     }
/// });
/// ```
pub const fn guard_fn<F>(f: F) -> FnGuard<F>
where
    F: Fn(&App, &NavigationRequest) -> NavigationAction + Send + Sync + 'static,
{
    FnGuard { f }
}

/// Guard created from a function or closure.
pub struct FnGuard<F> {
    f: F,
}

impl<F> RouteGuard for FnGuard<F>
where
    F: Fn(&App, &NavigationRequest) -> NavigationAction + Send + Sync + 'static,
{
    fn check(&self, cx: &App, request: &NavigationRequest) -> NavigationAction {
        (self.f)(cx, request)
    }
}

// ============================================================================
// AuthGuard
// ============================================================================

/// Function type for authentication checks.
///
/// Receives the app context and returns `true` if the user is authenticated.
pub type AuthCheckFn = Box<dyn Fn(&App) -> bool + Send + Sync>;

/// Authentication guard that checks if user is logged in.
///
/// # Example
///
/// ```no_run
/// use gpui::IntoElement;
/// use gpui_navigator::{Route, AuthGuard};
///
/// Route::new("/dashboard", |_, _cx, _params| gpui::div().into_any_element())
///     .guard(AuthGuard::new(|_cx| {
///         true // Replace with actual auth check
///     }, "/login"));
/// ```
pub struct AuthGuard {
    check_fn: AuthCheckFn,
    redirect_path: String,
}

impl AuthGuard {
    /// Create a new auth guard with a custom check function and redirect path.
    pub fn new<F>(check_fn: F, redirect_path: impl Into<String>) -> Self
    where
        F: Fn(&App) -> bool + Send + Sync + 'static,
    {
        Self {
            check_fn: Box::new(check_fn),
            redirect_path: redirect_path.into(),
        }
    }

    /// Create an auth guard that always allows access (for testing/development).
    #[cfg(debug_assertions)]
    #[must_use] 
    pub fn allow_all() -> Self {
        Self::new(|_| true, "/login")
    }

    /// Create an auth guard that always denies access (for testing/development).
    #[cfg(debug_assertions)]
    pub fn deny_all(redirect_path: impl Into<String>) -> Self {
        Self::new(|_| false, redirect_path)
    }
}

impl RouteGuard for AuthGuard {
    fn check(&self, cx: &App, _request: &NavigationRequest) -> NavigationAction {
        if (self.check_fn)(cx) {
            NavigationAction::Continue
        } else {
            NavigationAction::redirect_with_reason(&self.redirect_path, "Authentication required")
        }
    }

    fn name(&self) -> &'static str {
        "AuthGuard"
    }

    fn priority(&self) -> i32 {
        100
    }
}

// ============================================================================
// RoleGuard
// ============================================================================

/// Function type for extracting the current user's role.
///
/// Returns `None` if the user has no role or is not authenticated.
pub type RoleExtractorFn = Box<dyn Fn(&App) -> Option<String> + Send + Sync>;

/// Role-based authorization guard.
///
/// Checks if user has required role for accessing a route.
///
/// # Example
///
/// ```no_run
/// use gpui::IntoElement;
/// use gpui_navigator::{Route, RoleGuard};
///
/// Route::new("/admin", |_, _cx, _params| gpui::div().into_any_element())
///     .guard(RoleGuard::new(
///         |_cx| Some("admin".into()),
///         "admin",
///         Some("/forbidden"),
///     ));
/// ```
pub struct RoleGuard {
    role_extractor: RoleExtractorFn,
    required_role: String,
    redirect_path: Option<String>,
}

impl RoleGuard {
    /// Create a new role guard with a role extractor function.
    pub fn new<F>(
        role_extractor: F,
        required_role: impl Into<String>,
        redirect_path: Option<impl Into<String>>,
    ) -> Self
    where
        F: Fn(&App) -> Option<String> + Send + Sync + 'static,
    {
        Self {
            role_extractor: Box::new(role_extractor),
            required_role: required_role.into(),
            redirect_path: redirect_path.map(Into::into),
        }
    }
}

impl RouteGuard for RoleGuard {
    fn check(&self, cx: &App, _request: &NavigationRequest) -> NavigationAction {
        let has_role = (self.role_extractor)(cx).is_some_and(|role| role == self.required_role);

        if has_role {
            NavigationAction::Continue
        } else if let Some(redirect) = &self.redirect_path {
            NavigationAction::redirect_with_reason(
                redirect,
                format!("Requires '{}' role", self.required_role),
            )
        } else {
            NavigationAction::deny(format!("Missing required role: {}", self.required_role))
        }
    }

    fn name(&self) -> &'static str {
        "RoleGuard"
    }

    fn priority(&self) -> i32 {
        90
    }
}

// ============================================================================
// PermissionGuard
// ============================================================================

/// Function type for checking a specific permission.
///
/// Receives the app context and the permission string to check.
/// Returns `true` if the user holds the requested permission.
pub type PermissionCheckFn = Box<dyn Fn(&App, &str) -> bool + Send + Sync>;

/// Permission-based authorization guard.
///
/// Checks if user has a specific permission.
///
/// # Example
///
/// ```no_run
/// use gpui::IntoElement;
/// use gpui_navigator::{Route, PermissionGuard};
///
/// Route::new("/users/:id/delete", |_, _cx, _params| gpui::div().into_any_element())
///     .guard(PermissionGuard::new(
///         |_cx, _perm| true, // Replace with actual permission check
///         "users.delete",
///     ));
/// ```
pub struct PermissionGuard {
    check_fn: PermissionCheckFn,
    permission: String,
    redirect_path: Option<String>,
}

impl PermissionGuard {
    /// Create a new permission guard with a check function.
    pub fn new<F>(check_fn: F, permission: impl Into<String>) -> Self
    where
        F: Fn(&App, &str) -> bool + Send + Sync + 'static,
    {
        Self {
            check_fn: Box::new(check_fn),
            permission: permission.into(),
            redirect_path: None,
        }
    }

    /// Add a redirect path for when permission is denied.
    #[must_use]
    pub fn with_redirect(mut self, path: impl Into<String>) -> Self {
        self.redirect_path = Some(path.into());
        self
    }
}

impl RouteGuard for PermissionGuard {
    fn check(&self, cx: &App, _request: &NavigationRequest) -> NavigationAction {
        if (self.check_fn)(cx, &self.permission) {
            NavigationAction::Continue
        } else if let Some(redirect) = &self.redirect_path {
            NavigationAction::redirect_with_reason(
                redirect,
                format!("Missing permission: {}", self.permission),
            )
        } else {
            NavigationAction::deny(format!("Missing permission: {}", self.permission))
        }
    }

    fn name(&self) -> &'static str {
        "PermissionGuard"
    }

    fn priority(&self) -> i32 {
        80
    }
}

// ============================================================================
// Guard Composition
// ============================================================================

/// Combines multiple guards with AND logic.
///
/// All guards must return [`NavigationAction::Continue`] for navigation to proceed.
/// The first non-continue result is returned immediately (short-circuit).
///
/// Guards are executed in priority order (higher priority first).
///
/// # Example
///
/// ```no_run
/// use gpui_navigator::{Guards, AuthGuard, RoleGuard};
///
/// let guard = Guards::builder()
///     .guard(AuthGuard::new(|_| true, "/login"))
///     .guard(RoleGuard::new(|_| Some("admin".into()), "admin", None::<&str>))
///     .build();
/// ```
pub struct Guards {
    guards: Vec<Box<dyn RouteGuard>>,
}

impl Guards {
    /// Create a new AND composition from a vec of boxed guards.
    #[must_use] 
    pub fn new(guards: Vec<Box<dyn RouteGuard>>) -> Self {
        Self { guards }
    }

    /// Start building a guard composition.
    pub fn builder() -> GuardBuilder {
        GuardBuilder::new()
    }
}

impl RouteGuard for Guards {
    fn check(&self, cx: &App, request: &NavigationRequest) -> NavigationAction {
        let mut sorted: Vec<_> = self.guards.iter().collect();
        sorted.sort_by_key(|g| std::cmp::Reverse(g.priority()));

        for guard in sorted {
            let result = guard.check(cx, request);
            if !matches!(result, NavigationAction::Continue) {
                return result;
            }
        }
        NavigationAction::Continue
    }

    fn name(&self) -> &'static str {
        "Guards"
    }

    fn priority(&self) -> i32 {
        self.guards.iter().map(|g| g.priority()).max().unwrap_or(0)
    }
}

/// Builder for [`Guards`] with fluent API.
#[must_use]
pub struct GuardBuilder {
    guards: Vec<Box<dyn RouteGuard>>,
}

impl GuardBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self { guards: Vec::new() }
    }

    /// Add a guard to the composition.
    pub fn guard<G: RouteGuard>(mut self, guard: G) -> Self {
        self.guards.push(Box::new(guard));
        self
    }

    /// Build the final [`Guards`].
    #[must_use] 
    pub fn build(self) -> Guards {
        Guards::new(self.guards)
    }
}

impl Default for GuardBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// NotGuard
// ============================================================================

/// Inverts a guard result.
///
/// - `Continue` becomes `Deny`
/// - `Deny` becomes `Continue`
/// - `Redirect` is preserved as-is
///
/// # Example
///
/// ```no_run
/// use gpui_navigator::{NotGuard, AuthGuard};
///
/// // Allow only if NOT authenticated (e.g. for a login page)
/// let guard = NotGuard::new(AuthGuard::new(|_| true, "/login"));
/// ```
pub struct NotGuard {
    guard: Box<dyn RouteGuard>,
}

impl NotGuard {
    /// Create a new NOT guard wrapping the given guard.
    pub fn new<G: RouteGuard>(guard: G) -> Self {
        Self {
            guard: Box::new(guard),
        }
    }
}

impl RouteGuard for NotGuard {
    fn check(&self, cx: &App, request: &NavigationRequest) -> NavigationAction {
        match self.guard.check(cx, request) {
            NavigationAction::Continue => {
                NavigationAction::deny("Inverted: guard allowed but NOT expected")
            }
            NavigationAction::Deny { .. } => NavigationAction::Continue,
            redirect @ NavigationAction::Redirect { .. } => redirect,
        }
    }

    fn name(&self) -> &'static str {
        "NotGuard"
    }

    fn priority(&self) -> i32 {
        self.guard.priority()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(path: &str) -> NavigationRequest {
        NavigationRequest::new(path.to_string())
    }

    // --- RouteGuard trait basics ---

    #[test]
    fn test_guard_fn_helper() {
        let guard = guard_fn(|_cx, _req| NavigationAction::Continue);
        assert_eq!(guard.name(), "RouteGuard");
        assert_eq!(guard.priority(), 0);
    }

    // --- AuthGuard ---

    #[gpui::test]
    fn test_auth_guard_allows_authenticated(cx: &mut gpui::TestAppContext) {
        let guard = AuthGuard::new(|_| true, "/login");
        assert_eq!(guard.name(), "AuthGuard");
        assert_eq!(guard.priority(), 100);

        let request = make_request("/dashboard");
        let result = cx.update(|cx| guard.check(cx, &request));
        assert!(result.is_continue());
    }

    #[gpui::test]
    fn test_auth_guard_redirects_unauthenticated(cx: &mut gpui::TestAppContext) {
        let guard = AuthGuard::new(|_| false, "/login");
        let request = make_request("/dashboard");
        let result = cx.update(|cx| guard.check(cx, &request));

        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/login"));
    }

    // --- RoleGuard ---

    #[gpui::test]
    fn test_role_guard_allows_correct_role(cx: &mut gpui::TestAppContext) {
        let guard = RoleGuard::new(|_| Some("admin".to_string()), "admin", None::<String>);
        assert_eq!(guard.name(), "RoleGuard");
        assert_eq!(guard.priority(), 90);

        let request = make_request("/admin");
        let result = cx.update(|cx| guard.check(cx, &request));
        assert!(result.is_continue());
    }

    #[gpui::test]
    fn test_role_guard_with_redirect(cx: &mut gpui::TestAppContext) {
        let guard = RoleGuard::new(|_| Some("user".to_string()), "admin", Some("/403"));
        let request = make_request("/admin");
        let result = cx.update(|cx| guard.check(cx, &request));

        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/403"));
    }

    #[gpui::test]
    fn test_role_guard_deny_without_redirect(cx: &mut gpui::TestAppContext) {
        let guard = RoleGuard::new(|_| None, "admin", None::<String>);
        let request = make_request("/admin");
        let result = cx.update(|cx| guard.check(cx, &request));
        assert!(result.is_deny());
    }

    // --- PermissionGuard ---

    #[gpui::test]
    fn test_permission_guard_allows(cx: &mut gpui::TestAppContext) {
        let guard = PermissionGuard::new(|_, _| true, "users.delete");
        assert_eq!(guard.name(), "PermissionGuard");

        let request = make_request("/users/123/delete");
        let result = cx.update(|cx| guard.check(cx, &request));
        assert!(result.is_continue());
    }

    #[gpui::test]
    fn test_permission_guard_denies(cx: &mut gpui::TestAppContext) {
        let guard = PermissionGuard::new(|_, _| false, "users.delete");
        let request = make_request("/users/123/delete");
        let result = cx.update(|cx| guard.check(cx, &request));
        assert!(result.is_deny());
    }

    #[gpui::test]
    fn test_permission_guard_with_redirect(cx: &mut gpui::TestAppContext) {
        let guard = PermissionGuard::new(|_, _| false, "users.delete").with_redirect("/forbidden");
        let request = make_request("/users/123/delete");
        let result = cx.update(|cx| guard.check(cx, &request));

        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/forbidden"));
    }

    // --- Guards composition ---

    #[gpui::test]
    fn test_guards_all_pass(cx: &mut gpui::TestAppContext) {
        let guards = Guards::builder()
            .guard(AuthGuard::new(|_| true, "/login"))
            .guard(RoleGuard::new(
                |_| Some("admin".to_string()),
                "admin",
                None::<String>,
            ))
            .build();

        let request = make_request("/admin");
        let result = cx.update(|cx| guards.check(cx, &request));
        assert!(result.is_continue());
    }

    #[gpui::test]
    fn test_guards_one_fails(cx: &mut gpui::TestAppContext) {
        let guards = Guards::builder()
            .guard(AuthGuard::new(|_| true, "/login"))
            .guard(RoleGuard::new(|_| None, "admin", Some("/forbidden")))
            .build();

        let request = make_request("/admin");
        let result = cx.update(|cx| guards.check(cx, &request));
        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/forbidden"));
    }

    #[gpui::test]
    fn test_guards_priority_order(cx: &mut gpui::TestAppContext) {
        // Auth (priority 100) should run before Role (priority 90)
        // If auth fails, we should get auth's redirect, not role's
        let guards = Guards::builder()
            .guard(RoleGuard::new(|_| None, "admin", Some("/role-denied")))
            .guard(AuthGuard::new(|_| false, "/auth-denied"))
            .build();

        let request = make_request("/admin");
        let result = cx.update(|cx| guards.check(cx, &request));
        assert_eq!(result.redirect_path(), Some("/auth-denied"));
    }

    // --- NotGuard ---

    #[gpui::test]
    fn test_not_guard_inverts_allow(cx: &mut gpui::TestAppContext) {
        let guard = NotGuard::new(guard_fn(|_, _| NavigationAction::Continue));
        let request = make_request("/test");
        let result = cx.update(|cx| guard.check(cx, &request));
        assert!(result.is_deny());
    }

    #[gpui::test]
    fn test_not_guard_inverts_deny(cx: &mut gpui::TestAppContext) {
        let guard = NotGuard::new(guard_fn(|_, _| NavigationAction::deny("nope")));
        let request = make_request("/test");
        let result = cx.update(|cx| guard.check(cx, &request));
        assert!(result.is_continue());
    }

    #[gpui::test]
    fn test_not_guard_preserves_redirect(cx: &mut gpui::TestAppContext) {
        let guard = NotGuard::new(guard_fn(|_, _| NavigationAction::redirect("/somewhere")));
        let request = make_request("/test");
        let result = cx.update(|cx| guard.check(cx, &request));
        assert!(result.is_redirect());
        assert_eq!(result.redirect_path(), Some("/somewhere"));
    }
}
