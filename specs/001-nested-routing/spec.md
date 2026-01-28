# Feature Specification: Nested Routing Improvements

**Feature Branch**: `001-nested-routing`  
**Created**: 2026-01-28  
**Status**: Draft  
**Input**: User description: "Improve nested routing logic with comprehensive tests, bug fixes, performance optimizations, and documentation. Fix current issues with route resolution, parameter inheritance, index routes, and path normalization. Add unit and integration tests. Optimize performance using Rust idioms (reduce allocations, use Cow, efficient matching). Document all public APIs with examples."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reliable Nested Route Resolution (Priority: P1)

As a GPUI Navigator user, I need nested routes (parent/child hierarchies) to resolve correctly so that my dashboard layouts with child pages work reliably without hanging or displaying wrong content.

**Why this priority**: Core functionality blocker. The library's nested routing feature is currently unreliable, breaking the primary use case (layouts with child routes). Without this working, users cannot build real applications.

**Independent Test**: Can be fully tested by creating a parent route with 3 child routes, navigating between them, and verifying correct child renders in RouterOutlet without errors or hangs. Delivers working nested navigation.

**Acceptance Scenarios**:

1. **Given** a parent route `/dashboard` with child routes `overview`, `analytics`, `settings`, **When** user navigates to `/dashboard/overview`, **Then** the overview page renders within the dashboard layout
2. **Given** user is on `/dashboard/analytics`, **When** user navigates to `/dashboard/settings`, **Then** the settings page replaces analytics within the same parent layout
3. **Given** a parent route with an index route (empty path child), **When** user navigates to parent path exactly `/dashboard`, **Then** the index child route renders automatically
4. **Given** deeply nested routes (3+ levels), **When** user navigates to deepest path like `/dashboard/users/123/profile`, **Then** each level's RouterOutlet renders its corresponding child correctly

---

### User Story 2 - Correct Parameter Inheritance (Priority: P2)

As a GPUI Navigator user, I need route parameters to be inherited correctly from parent to child routes so that child pages can access parent parameters (e.g., workspace ID accessible to all workspace sub-pages).

**Why this priority**: Essential for real-world multi-level navigation (workspace > project > task). Without parameter inheritance, users must duplicate parameters at every level, breaking DRY principles.

**Independent Test**: Create parent route `/workspace/:workspace_id` with child route `projects/:project_id`, navigate to `/workspace/abc/projects/123`, verify child can access both workspace_id=abc and project_id=123. Delivers parameter passing.

**Acceptance Scenarios**:

1. **Given** parent route `/workspace/:workspace_id` and child route `projects/:project_id`, **When** navigating to `/workspace/abc/projects/123`, **Then** child route receives merged parameters {workspace_id: "abc", project_id: "123"}
2. **Given** nested parameters conflict (same name), **When** child parameter matches parent parameter name, **Then** child parameter value takes precedence
3. **Given** a 3-level nested route with parameters at each level, **When** navigating to deepest route, **Then** all ancestor parameters are accessible to the deepest child

---

### User Story 3 - Path Normalization Consistency (Priority: P2)

As a GPUI Navigator user, I need routes with various path formats (trailing slashes, leading slashes, empty paths) to be normalized consistently so that `/dashboard`, `/dashboard/`, and `dashboard` all work reliably.

**Why this priority**: Prevents frustrating edge-case bugs where navigation works in some cases but fails in others due to slash inconsistencies. Improves developer experience.

**Independent Test**: Define routes with mixed path formats, navigate using different variations, verify all resolve to same route. Delivers path format flexibility.

**Acceptance Scenarios**:

1. **Given** route defined as `/dashboard`, **When** user navigates to `/dashboard/` (trailing slash), **Then** route matches successfully
2. **Given** child route defined as `settings` under parent `/dashboard`, **When** building full path, **Then** result is `/dashboard/settings` (single slash separator)
3. **Given** index route with empty path `""`, **When** matched against parent path, **Then** parent path alone matches the index route
4. **Given** route with leading slash `/about`, **When** compared to path without leading slash `about`, **Then** paths are treated as equivalent after normalization

---

### User Story 4 - Named Outlet Support (Priority: P3)

As a GPUI Navigator user, I need to define multiple named outlets (e.g., main content + sidebar) so that different child route hierarchies can render in parallel within the same parent.

**Why this priority**: Advanced feature for complex layouts. Not essential for basic nested routing but enables sophisticated multi-panel UIs.

**Independent Test**: Create parent with named children for outlets "main" and "sidebar", render two RouterOutlets with different names, verify each renders its corresponding child. Delivers multi-outlet layouts.

**Acceptance Scenarios**:

1. **Given** parent route with named children for outlet "sidebar", **When** RouterOutlet::named("sidebar") is rendered, **Then** it displays only the sidebar child routes
2. **Given** parent has both default children and named children, **When** default RouterOutlet is rendered alongside named outlet, **Then** each outlet renders its independent child hierarchy
3. **Given** user navigates to route with named outlet, **When** route changes, **Then** only the affected outlet updates (others remain unchanged)

---

### User Story 5 - Performance Optimization (Priority: P3)

As a GPUI Navigator user, I need route resolution to be fast and allocation-efficient so that navigation feels instant even with complex route trees (50+ routes, 5+ nesting levels).

**Why this priority**: Performance is important but not blocking. Current code works but may have inefficiencies (unnecessary String allocations, repeated path parsing).

**Independent Test**: Benchmark route resolution with 100 routes across 5 nesting levels, verify resolution completes in <1ms and allocations are minimized. Delivers performance confidence.

**Acceptance Scenarios**:

1. **Given** a route tree with 100 routes, **When** resolving any route, **Then** resolution completes in under 1 millisecond
2. **Given** paths that can be borrowed (static strings), **When** building child paths, **Then** Cow<str> avoids unnecessary allocations
3. **Given** repeated route lookups for the same path, **When** resolution runs multiple times, **Then** performance remains consistent (no degradation)

---

### Edge Cases

- What happens when navigating to a parent route with no index child defined?
- How does the system handle conflicting parameter names between parent and child?
- What happens when a child route path contains multiple segments (e.g., `settings/profile`)?
- How are empty paths, "/", and missing paths handled differently?
- What happens when RouterOutlet is rendered for a parent with no children?
- How does the system handle navigation to deeply nested routes that skip intermediate levels?
- What happens when named outlet children are requested but the outlet name doesn't exist?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST correctly resolve child routes within nested parent routes for up to 5 nesting levels
- **FR-002**: System MUST merge route parameters from parent routes into child route parameters
- **FR-003**: System MUST normalize paths consistently (handle leading/trailing slashes, empty paths)
- **FR-004**: System MUST support index routes (default children with empty or "/" path)
- **FR-005**: System MUST support named outlets with independent child route hierarchies
- **FR-006**: System MUST handle edge cases gracefully (no index route, conflicting parameters, malformed paths)
- **FR-007**: Route resolution MUST complete within 1ms for typical route trees (≤50 routes, ≤5 levels)
- **FR-008**: Path building MUST avoid unnecessary allocations using Cow<str> for borrowed strings
- **FR-009**: All public APIs MUST have rustdoc comments with code examples
- **FR-010**: System MUST provide unit tests for route matching logic
- **FR-011**: System MUST provide integration tests for nested navigation scenarios
- **FR-012**: System MUST log trace-level diagnostics for route resolution debugging

### Key Entities

- **Route**: Represents a navigation destination with path, builder function, children, named_children, transitions, and guards. Core immutable routing configuration.
- **RouteParams**: Key-value map of route parameters extracted from URL segments (e.g., {id: "123"}). Merged from parent to child.
- **ResolvedChildRoute**: Tuple of (matched child Route, merged RouteParams). Result of successful child route resolution.
- **RouterOutlet**: GPUI component that renders the current/child route. Can be named for multi-outlet layouts.
- **Path Segment**: Individual component of a route path (e.g., "/dashboard/settings" has segments ["dashboard", "settings"])

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All nested_demo.rs example scenarios execute without errors or hangs
- **SC-002**: Route resolution completes in under 1 millisecond for route trees with 50 routes across 5 nesting levels
- **SC-003**: All 5 user stories have passing integration tests demonstrating the scenario
- **SC-004**: Code coverage for src/nested.rs reaches 80%+ with unit and integration tests
- **SC-005**: Zero unnecessary String allocations when resolving routes with static paths (measurable via allocation profiler)
- **SC-006**: All public functions in src/nested.rs have rustdoc comments with runnable examples
- **SC-007**: Developers can create 3-level nested routes in under 10 lines of code
- **SC-008**: Navigation between nested routes feels instant (60fps transitions, no jank)

## Assumptions

- Users are working with GPUI 0.2.x framework
- Typical applications have ≤50 total routes with ≤5 nesting levels (edge cases like 1000+ routes are out of scope)
- Route definitions are static at initialization (no dynamic route addition after router initialization)
- Route resolution happens on main thread (no multi-threading complications)
- Developers are familiar with React-Router or Go-Router patterns (nested routing is not a new concept to them)
- Performance targets are based on single-threaded route resolution without caching (caching is optional feature)

## Out of Scope

- Dynamic route addition/removal after router initialization
- Server-side rendering (SSR) of nested routes
- Route lazy-loading or code-splitting
- Route animation/transition implementation (focus is on resolution logic, not rendering)
- Browser history integration (GPUI is desktop-focused, not web)
- Route authentication/authorization guards (separate feature)
- Route caching optimization (optional feature flag)
