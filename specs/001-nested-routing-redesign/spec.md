# Feature Specification: Nested Routing Architecture Redesign

**Feature Branch**: `001-nested-routing-redesign`  
**Created**: 2026-01-28  
**Status**: Draft  
**Input**: User description: "мой nested не работает и создает очень много различных проблем я хочу пересмотреть архитектуру более правильную и рабочую и удобную для разрабочиков решение. щас основные проблемы не рендерится по цепочке, рекурсии и проблемы с statefull компонентами"

## Clarifications

### Session 2026-01-28

- Q: How should the system manage stateful component lifecycle when routes become inactive? → A: Cache with LRU eviction after N inactive routes + explicit cleanup via Navigator methods
- Q: What is the default LRU cache size for inactive stateful components? → A: 10 routes
- Q: What should happen when a route component throws an error during rendering? → A: Show error UI in outlet, parent layout remains functional (isolated error boundary)
- Q: How to handle multiple navigation events in rapid succession? → A: Cancel previous navigation, start new one (cancellation-based approach)

## User Scenarios & Testing

### User Story 1 - Simple Nested Routes with Layouts (Priority: P1)

Developer creates a parent route with a layout component that contains child routes. When navigating between child routes, the parent layout remains visible and only the child content changes.

**Why this priority**: This is the core use case for nested routing - maintaining consistent layouts while changing content. Without this working, nested routing provides no value.

**Independent Test**: Create a dashboard route at `/dashboard` with a sidebar layout and three child routes: `/dashboard/overview`, `/dashboard/analytics`, `/dashboard/settings`. Navigate between children - sidebar should remain visible, only content area should change.

**Acceptance Scenarios**:

1. **Given** a parent route with layout and multiple child routes, **When** user navigates to parent route, **Then** parent layout renders with default child (index route) displayed in outlet
2. **Given** user is on a child route, **When** user navigates to sibling child route, **Then** parent layout remains mounted, only child content changes
3. **Given** nested route hierarchy, **When** route parameters change (e.g., `/products/:id`), **Then** parent layout persists, child component re-renders with new params

---

### User Story 2 - Stateful Components Maintain State (Priority: P1)

Developer creates stateful route components (components with internal state like form data, scroll position, etc.). When navigating away and back to a route, the component state should be preserved within LRU cache limits (default 10 routes).

**Why this priority**: Stateful components are essential for real applications (forms, lists with filters, etc.). If state resets on every navigation, the routing system is unusable for production apps.

**Independent Test**: Create a counter component at `/counter` with increment/decrement buttons. Navigate to `/counter`, increment to 5, navigate away to `/home`, navigate back to `/counter` - counter should still show 5 (assuming under 10-route cache limit).

**Acceptance Scenarios**:

1. **Given** a stateful route component within cache limit, **When** user modifies component state and navigates away, **Then** component state is preserved in LRU cache
2. **Given** user returns to a previously visited route within cache, **When** route is re-rendered, **Then** component displays with previous state intact
3. **Given** multiple instances of same route with different params (e.g., `/user/1`, `/user/2`), **When** navigating between them, **Then** each instance maintains separate state in cache
4. **Given** cache exceeds 10 inactive routes, **When** LRU eviction triggers, **Then** oldest inactive component is destroyed and removed from cache
5. **Given** developer calls Navigator cleanup method for specific route, **When** cleanup executes, **Then** component state is immediately destroyed regardless of cache

---

### User Story 3 - Deep Nested Hierarchies (Priority: P2)

Developer creates multi-level nested routes (e.g., `/app/workspace/project/task/details`) with layouts at each level. Navigation should work smoothly without infinite render loops or recursion errors.

**Why this priority**: Real applications often need 3+ levels of nesting (app shell → workspace → project → item). This validates the architecture scales properly.

**Independent Test**: Create 4-level hierarchy: root layout → workspace layout → project layout → task page. Navigate through the hierarchy - all layouts should render correctly, navigation should be responsive.

**Acceptance Scenarios**:

1. **Given** multi-level nested routes, **When** user navigates to deeply nested route, **Then** all parent layouts render in correct order without recursion errors
2. **Given** deep hierarchy, **When** user navigates between routes at same level, **Then** system renders efficiently without re-rendering entire tree
3. **Given** deeply nested route, **When** component renders, **Then** no infinite render loops occur

---

### User Story 4 - Index Routes as Defaults (Priority: P2)

Developer defines index routes (routes with empty path "") as default children for parent routes. When navigating to parent route without specifying child, index route should display automatically.

**Why this priority**: Index routes are a standard routing pattern that simplifies navigation - users can go to `/dashboard` instead of `/dashboard/overview`.

**Independent Test**: Define `/dashboard` with index route pointing to OverviewPage. Navigate to `/dashboard` - OverviewPage should display automatically without needing `/dashboard/overview`.

**Acceptance Scenarios**:

1. **Given** parent route with index child, **When** user navigates to parent path only, **Then** index child renders automatically
2. **Given** index route at root level, **When** app starts at `/`, **Then** root index route displays
3. **Given** nested index routes at multiple levels, **When** user navigates to any parent, **Then** correct index child renders at that level

---

### User Story 5 - Route Parameters Inheritance (Priority: P3)

Developer defines route parameters at parent level that should be accessible in child routes without re-declaring them.

**Why this priority**: Parameter inheritance reduces boilerplate and keeps route definitions DRY. It's a convenience feature that improves developer experience.

**Independent Test**: Define `/workspace/:workspaceId/project/:projectId` where child routes can access both `workspaceId` and `projectId`. Navigate to `/workspace/123/project/456/settings` - settings page should receive both params.

**Acceptance Scenarios**:

1. **Given** parent route with path params, **When** child route renders, **Then** child receives parent params in addition to own params
2. **Given** multi-level params, **When** deeply nested child renders, **Then** child receives all ancestor params merged
3. **Given** param collision (parent and child have same param name), **When** child renders, **Then** child param takes precedence

---

### Edge Cases

- What happens when RouterOutlet is used in a route without children? (Should render nothing gracefully)
- How does system handle circular route dependencies? (Should detect and error at registration time)
- What happens when navigating to non-existent route? (Should render not-found page)
- How are route transitions handled when navigation occurs during active transition? (Cancel previous navigation, start new one)
- What happens when component constructor throws error? (Display error UI in outlet, parent layout remains functional via error boundary)
- How does system handle very rapid navigation (user clicks multiple links quickly)? (Cancel previous navigation, process only the latest)
- What happens when LRU cache is full and new route needs caching? (Evict least-recently-used inactive component)
- What happens when developer explicitly cleans up cached route? (Immediate destruction regardless of LRU position)

## Requirements

### Functional Requirements

- **FR-001**: System MUST support arbitrary nesting depth of routes (minimum 5 levels deep)
- **FR-002**: System MUST preserve stateful component instances in LRU cache (default 10 inactive routes) when navigating away, and restore them on return
- **FR-003**: System MUST render parent layouts before child content in hierarchical order
- **FR-004**: System MUST prevent infinite render loops when RouterOutlet is used in nested layouts
- **FR-005**: System MUST support index routes (empty path "") as default children
- **FR-006**: System MUST allow both stateless (function-based) and stateful (struct-based) route components
- **FR-007**: System MUST propagate route parameters from parent to child routes
- **FR-008**: System MUST detect and prevent circular route dependencies at registration time
- **FR-009**: System MUST evict oldest inactive stateful component when LRU cache exceeds capacity (configurable, default 10)
- **FR-010**: System MUST provide Navigator API methods for explicit component state cleanup (bypassing LRU cache)
- **FR-011**: System MUST support multiple RouterOutlet instances in same parent (named outlets)
- **FR-012**: System MUST handle navigation events efficiently without blocking UI thread
- **FR-013**: System MUST cancel in-flight navigation when new navigation starts (cancellation-based approach)
- **FR-014**: System MUST catch component rendering errors and display error UI in outlet while preserving parent layout functionality
- **FR-015**: System MUST provide clear error messages when route configuration is invalid

### Key Entities

- **Route**: Represents a path segment in the routing hierarchy with optional builder function, children array, and configuration
- **RouterOutlet**: Placeholder component that renders matched child routes, manages child component lifecycle and error boundaries
- **RouterState**: Maintains current navigation state including active path, matched routes, route parameters, and LRU component cache
- **RouteParams**: Key-value map of path parameters extracted from URL, inherited from parent routes
- **Navigator**: Provides API for programmatic navigation (push, replace, back, forward) and explicit component cleanup methods
- **ComponentCache**: LRU cache storing inactive stateful component instances (default capacity: 10 routes)

## Success Criteria

### Measurable Outcomes

- **SC-001**: Developers can create nested routes up to 5 levels deep without encountering render errors or infinite loops
- **SC-002**: Stateful components maintain state across navigation cycles when within LRU cache capacity (10 routes default)
- **SC-003**: Navigation between sibling child routes completes in under 16ms (single frame at 60fps)
- **SC-004**: Route registration phase detects circular dependencies and reports clear error before runtime
- **SC-005**: System renders nested layouts in correct hierarchical order 100% of the time
- **SC-006**: Component rendering errors are caught and isolated to outlet area, parent layouts remain interactive 100% of the time
- **SC-007**: Rapid navigation (5+ clicks/second) processes only final destination without rendering intermediate states
- **SC-008**: LRU cache eviction occurs within 5ms when capacity exceeded
- **SC-009**: Documentation includes working examples for all common patterns (layouts, params, stateful components, index routes, error handling, cache management)
- **SC-010**: Unit tests cover core scenarios with >80% code coverage
- **SC-011**: Integration tests validate all user stories pass without manual intervention

## Assumptions

- GPUI 0.2.x framework provides reactive state management and component lifecycle hooks
- Developers using this library have basic understanding of routing concepts (paths, params, navigation)
- Performance target is desktop applications (60fps rendering)
- Route configuration happens at app startup, not dynamically at runtime
- Path matching follows standard URL patterns (segments separated by `/`, params prefixed with `:`)
- Default behavior for missing routes is to display not-found page (no automatic redirects)
- LRU cache size of 10 routes is reasonable default for desktop apps (covers typical tab/back navigation patterns)
- Component state size is reasonable (not GBs) for in-memory caching

## Out of Scope

- Browser history integration (back/forward buttons) - future enhancement
- Lazy loading of route components - future enhancement  
- Route guards and middleware - exists as separate feature
- Query parameter handling - exists as separate feature
- Hash-based routing - only path-based routing in scope
- Server-side rendering - client-side only
- Animated route transitions - exists as separate feature
- Deep linking from external sources - future enhancement
- Configurable LRU cache size at runtime - only default 10 in initial version
- Persistent state across app restarts - only in-memory cache
