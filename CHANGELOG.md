# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `Route::component_with_context` and `Route::component_with_params_and_context`: stateful routes whose
  factory receives `&mut Window` and `&mut Context<T>`, so pages can create child entities such as
  gpui-component's `InputState` (#1)
- README note on wrapping the window root in `gpui_component::Root`
- Initial public release preparation
- Comprehensive documentation for all public APIs
- LICENSE-MIT and LICENSE-APACHE files
- CHANGELOG.md following Keep a Changelog format
- CI/CD configuration for GitHub Actions

### Changed
- `AuthGuard` now requires a check function instead of using a placeholder
- `RoleGuard` now requires a role extractor function for proper configuration
- `PermissionGuard` now requires a permission check function
- Improved error messages for guard failures
- Updated Cargo.toml with production-ready metadata

### Fixed
- `Route::component_with_params` cache keys no longer depend on `HashMap` iteration order, so routes with
  several parameters reuse their cached entity instead of recreating it
- CI workflow: use `dtolnay/rust-toolchain`, install `libxkbcommon-x11-dev`, run the MSRV job on the declared MSRV
- MSRV raised to 1.88 to match what the gpui 0.2 dependency tree actually requires
- Guards no longer use hardcoded `false` returns
- Removed dead code warnings in nested route cache
- Fixed clippy warnings throughout the codebase

## [0.1.0] - 2024-01-01

### Added
- **Core Router**
  - `GlobalRouter` for application-wide routing state
  - `RouterState` for managing navigation history
  - `Navigator` with Flutter-style navigation API
  - `init_router` for easy router initialization

- **Route Definition**
  - `Route` builder with fluent API
  - `RouteConfig` for route metadata
  - Named routes with `NamedRouteRegistry`
  - Route parameters with type-safe extraction
  - Wildcard and pattern matching support

- **Transitions**
  - `Transition::fade()` for opacity animations
  - `Transition::slide_left/right/up/down()` for directional slides
  - `Transition::zoom_in/zoom_out()` for scale animations
  - Custom transition support via `TransitionAnimation` trait

- **Nested Routing**
  - `RouterOutlet` component for child route rendering
  - Named outlets for multiple content areas
  - `RouteCache` for optimized route resolution
  - Parent/child route hierarchy support

- **Guards**
  - `AuthGuard` for authentication checks
  - `RoleGuard` for role-based authorization
  - `PermissionGuard` for permission-based access control
  - `Guards` for composing multiple guards (AND logic)
  - `NotGuard` for inverting guard results
  - `guard_fn` helper for creating guards from closures

- **Middleware**
  - `RouteMiddleware` trait for before/after navigation hooks
  - `middleware_fn` helper for creating middleware from functions
  - Priority-based middleware execution order

- **Lifecycle Hooks**
  - `RouteLifecycle` trait for route lifecycle management
  - `on_enter` hook for route initialization
  - `on_exit` hook for cleanup
  - `can_deactivate` hook for navigation confirmation

- **Error Handling**
  - `NavigationError` for typed error handling
  - `NavigationResult` for navigation outcomes
  - `ErrorHandlers` for custom 404 and error pages

- **Parameters**
  - `RouteParams` for path parameters
  - `QueryParams` for query string parsing
  - Type-safe parameter extraction with `get_as<T>()`

- **Widgets**
  - `RouterOutlet` for outlet rendering
  - `RouterLink` for declarative navigation links
  - Helper functions: `router_outlet()`, `router_link()`

- **Advanced Matching**
  - `RoutePattern` for complex route patterns
  - `Segment` types: Static, Param, Optional, Wildcard
  - `Constraint` validation: Numeric, UUID, Pattern

### Documentation
- Comprehensive README with examples
- Doc comments for all public APIs
- Example application: `transition_demo`

[Unreleased]: https://github.com/nicholasoxford/gpui-navigator/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/nicholasoxford/gpui-navigator/releases/tag/v0.1.0