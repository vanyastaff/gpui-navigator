# Tasks: Nested Routing Architecture Redesign

**Feature**: 001-nested-routing-redesign  
**Input**: Design documents from `specs/001-nested-routing-redesign/`  
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅, quickstart.md ✅

**Organization**: Tasks grouped by user story priority (P1 → P2 → P3) to enable independent implementation and testing.

**Tests**: Included for validation (SC-007 requires >80% coverage, SC-008 requires integration tests)

---

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: User story this belongs to (US1, US2, US3, US4, US5)
- Exact file paths included in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [X] T001 Review existing codebase structure in src/ to understand current routing implementation
- [X] T002 [P] Add optional dependencies to Cargo.toml: lru = "0.12" (feature = "cache"), matchit = "0.8" (optional)
- [X] T003 [P] Create new source files per quickstart.md: src/matching.rs, src/cache.rs (feature-gated)
- [X] T004 Create tests/unit/ directory structure with matching.rs, params.rs, cache.rs, nested.rs
- [X] T005 Create tests/integration/ directory and nested_routing.rs file

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core routing infrastructure that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [ ] T006 Implement segment-based path matching in src/matching.rs: split path by '/', match literal and :param segments
- [ ] T007 Implement RouteParams merge function in src/params.rs: merge parent + child params, child overrides on collision
- [ ] T008 Update Route struct in src/route.rs: ensure Arc<Route> wrapping, immutable children vec
- [ ] T009 Add navigation_id: AtomicUsize field to RouterState in src/state.rs for cancellation tracking
- [ ] T010 [P] Implement two-phase rendering pattern in RouterOutlet (src/widgets.rs): Phase 1 resolve (immutable), Phase 2 build (mutable)
- [ ] T011 Write unit tests for segment matching in tests/unit/matching.rs: exact match, param extraction, no match scenarios
- [ ] T012 [P] Write unit tests for param merging in tests/unit/params.rs: parent+child, collision handling, empty params

**Checkpoint**: Foundation ready - core routing works, user stories can now build on this

---

## Phase 3: User Story 1 - Simple Nested Routes with Layouts (Priority: P1) 🎯 MVP

**Goal**: Parent routes with layouts contain child routes. Navigating between children preserves parent layout, only child content changes.

**Independent Test**: Create `/dashboard` with sidebar, 3 children (`/dashboard/overview`, `/dashboard/analytics`, `/dashboard/settings`). Navigate between children - sidebar stays, content changes.

### Implementation for User Story 1

- [ ] T013 [P] [US1] Implement hierarchical route resolution in src/nested.rs: resolve_child_route() function that consumes parent segments
- [ ] T014 [P] [US1] Implement find_index_route() in src/nested.rs: finds child with empty path "" as default
- [ ] T015 [US1] Update RouterOutlet::render() in src/widgets.rs: use resolve_child_route() for nested resolution
- [ ] T016 [US1] Add find_parent_for_outlet() helper in src/nested.rs: locates parent route for given outlet
- [ ] T017 [US1] Update nested_demo.rs example: create DashboardLayout with sidebar and 3 child routes (overview, analytics, settings)
- [ ] T018 [US1] Add path change detection in RouterOutlet (src/widgets.rs): only update state if current_path != new_path
- [ ] T019 [US1] Write unit tests for hierarchical resolution in tests/unit/nested.rs: parent+child matching, index route selection
- [ ] T020 [US1] Write integration test in tests/integration/nested_routing.rs: navigate /dashboard → /dashboard/analytics, verify layout persists

**Checkpoint**: User Story 1 complete - nested routes with persistent layouts work

---

## Phase 4: User Story 2 - Stateful Components Maintain State (Priority: P1) 🎯 MVP

**Goal**: Stateful route components (counters, forms) preserve state when navigating away and back, within LRU cache limits (10 routes).

**Independent Test**: Create `/counter` with increment button. Navigate to `/counter`, increment to 5, navigate to `/home`, return to `/counter` - shows 5.

### Implementation for User Story 2

- [ ] T021 [P] [US2] Implement ComponentCache struct in src/cache.rs (feature-gated): HashMap + VecDeque for LRU, capacity=10 default
- [ ] T022 [P] [US2] Implement ComponentCache::get_or_insert() in src/cache.rs: hit → move to back, miss → evict oldest if full
- [ ] T023 [US2] Add component_cache field to RouterState in src/state.rs (feature-gated with #[cfg(feature = "cache")])
- [ ] T024 [US2] Implement Route::component() method in src/route.rs: uses window.use_keyed_state() with route-specific key
- [ ] T025 [US2] Implement Route::component_with_params() in src/route.rs: key includes params for separate instances per param combo
- [ ] T026 [US2] Add Navigator::clear_cache() method in src/navigator.rs (feature-gated): explicit cache removal for path
- [ ] T027 [US2] Add Navigator::clear_all_cache() in src/navigator.rs (feature-gated): clears entire cache
- [ ] T028 [US2] Create CounterPage example component in examples/nested_demo.rs with stateful counter
- [ ] T029 [US2] Write unit tests for LRU cache in tests/unit/cache.rs: insertion, eviction, hit/miss, explicit removal
- [ ] T030 [US2] Write integration test in tests/integration/nested_routing.rs: counter state preserved across navigation

**Checkpoint**: User Story 2 complete - stateful components work, state cached within LRU limits

---

## Phase 5: User Story 3 - Deep Nested Hierarchies (Priority: P2)

**Goal**: Multi-level nested routes (4+ levels) render correctly without infinite loops or recursion errors.

**Independent Test**: Create 4-level hierarchy `/app/workspace/:workspaceId/project/:projectId/task/:taskId`. Navigate through - all layouts render, no loops.

### Implementation for User Story 3

- [ ] T031 [P] [US3] Add recursion depth limit check in src/nested.rs: max 10 levels, return error if exceeded
- [ ] T032 [P] [US3] Optimize route resolution path in src/matching.rs: early exit on segment count mismatch
- [ ] T033 [US3] Add performance logging in src/widgets.rs: log resolution time if >1ms (debug mode)
- [ ] T034 [US3] Create deep hierarchy example in examples/nested_demo.rs: AppLayout → WorkspaceLayout → ProjectLayout → TaskPage
- [ ] T035 [US3] Write integration test in tests/integration/nested_routing.rs: navigate 4-level hierarchy, measure resolution <1ms
- [ ] T036 [US3] Add stress test in tests/integration/nested_routing.rs: rapid navigation (10 navigations/second), no loops

**Checkpoint**: User Story 3 complete - deep nesting works efficiently without errors

---

## Phase 6: User Story 4 - Index Routes as Defaults (Priority: P2)

**Goal**: Index routes (empty path "") render automatically when navigating to parent path without specifying child.

**Independent Test**: Define `/dashboard` with index route. Navigate to `/dashboard` - index child renders without needing `/dashboard/overview`.

### Implementation for User Story 4

- [ ] T037 [P] [US4] Update find_index_route() in src/nested.rs: prioritize index routes (path="") when no exact child match
- [ ] T038 [P] [US4] Add index route validation in src/route.rs: warn if parent has no index and multiple children (ambiguous default)
- [ ] T039 [US4] Update resolve_child_route() in src/nested.rs: if remaining path empty, return index child or None
- [ ] T040 [US4] Add root-level index route support in src/state.rs: RouterState::find_route() checks for "" route at root
- [ ] T041 [US4] Update nested_demo.rs: add index routes at root ("/") and for dashboard ("")
- [ ] T042 [US4] Write unit test in tests/unit/nested.rs: parent with index child, navigate to parent path, verify index renders
- [ ] T043 [US4] Write integration test in tests/integration/nested_routing.rs: navigate "/" and "/dashboard", verify index routes render

**Checkpoint**: User Story 4 complete - index routes work at all levels

---

## Phase 7: User Story 5 - Route Parameters Inheritance (Priority: P3)

**Goal**: Child routes inherit parent route parameters without re-declaring them. Collision handling: child overrides parent.

**Independent Test**: Define `/workspace/:workspaceId/project/:projectId/settings`. Navigate to `/workspace/123/project/456/settings` - settings receives both params.

### Implementation for User Story 5

- [ ] T044 [P] [US5] Update merge_params() in src/params.rs: iterate parent params, add to result, then child params (overriding)
- [ ] T045 [P] [US5] Add RouteParams::from_path() helper in src/params.rs: extract params from path given pattern
- [ ] T046 [US5] Update resolve_child_route() in src/nested.rs: pass merged params (parent + child) to RouteMatch
- [ ] T047 [US5] Add param collision logging in src/nested.rs: warn when child param shadows parent param (debug mode)
- [ ] T048 [US5] Create multi-param example in examples/nested_demo.rs: WorkspaceLayout (workspaceId) → ProjectLayout (projectId) → SettingsPage
- [ ] T049 [US5] Write unit test in tests/unit/params.rs: multi-level param merging, verify child overrides parent on collision
- [ ] T050 [US5] Write integration test in tests/integration/nested_routing.rs: navigate multi-param route, verify all params accessible in child

**Checkpoint**: User Story 5 complete - parameter inheritance works correctly

---

## Phase 8: Error Handling & Edge Cases

**Purpose**: Implement error boundaries, not-found pages, validation per spec requirements

- [ ] T051 [P] Implement error boundary in RouterOutlet (src/widgets.rs): catch builder panics, display error UI, keep parent layout
- [ ] T052 [P] Create NotFoundPage component in src/widgets.rs: production-ready 404 page with home link
- [ ] T053 Add Router::not_found() configuration in src/state.rs: custom 404 route registration
- [ ] T054 Implement circular dependency detection in src/route.rs: validate route tree during construction, panic with clear message
- [ ] T055 Add navigation cancellation in src/state.rs: check navigation_id before completing navigation, skip if stale
- [ ] T056 Write unit test in tests/unit/route.rs: circular dependency detection catches cycles
- [ ] T057 Write integration test in tests/integration/nested_routing.rs: rapid navigation cancels previous, only final renders

---

## Phase 9: Performance Optimization

**Purpose**: Ensure SC-003 (<16ms navigation), SC-008 (<5ms cache eviction) targets met

- [ ] T058 [P] Profile route resolution in src/matching.rs: use Instant::now() to measure resolution time
- [ ] T059 [P] Add caching for route lookups in src/state.rs: HashMap<String, Arc<Route>> for O(1) path → route
- [ ] T060 Optimize segment splitting in src/matching.rs: pre-split paths during route construction, store in Route
- [ ] T061 Benchmark cache eviction in tests/unit/cache.rs: ensure eviction completes <5ms even with 1000 entries
- [ ] T062 Add performance integration test in tests/integration/nested_routing.rs: navigate 100 times, average <16ms

---

## Phase 10: Documentation & Examples

**Purpose**: Meet SC-006 (working examples), SC-009 (complete documentation) requirements

- [ ] T063 [P] Update examples/nested_demo.rs: comprehensive example with all 5 user stories demonstrated
- [ ] T064 [P] Create examples/stateful_demo.rs: showcases Route::component() with various stateful components
- [ ] T065 [P] Add rustdoc comments to all public APIs in src/route.rs, src/navigator.rs, src/widgets.rs
- [ ] T066 [P] Write quickstart guide in README.md: basic example, API overview, feature flags
- [ ] T067 [P] Add migration guide in CHANGELOG.md: breaking changes from 0.1.x, migration steps
- [ ] T068 [P] Document error handling patterns in docs/: error boundaries, not-found pages, validation errors
- [ ] T069 Add API reference documentation in docs/api.md: all Route methods, Navigator methods, component APIs

---

## Phase 11: Testing & Validation

**Purpose**: Achieve SC-007 (>80% coverage), SC-008 (integration tests pass), validate all acceptance scenarios

- [ ] T070 Run unit tests: `cargo test --lib` - verify all unit tests pass
- [ ] T071 Run integration tests: `cargo test --test nested_routing` - verify all user story scenarios pass
- [ ] T072 Run examples: `cargo run --example nested_demo` - manual validation of all 5 user stories
- [ ] T073 Run coverage report: `cargo tarpaulin` or `cargo llvm-cov` - verify >80% coverage
- [ ] T074 Validate acceptance scenarios from spec.md: check each Given/When/Then manually
- [ ] T075 Performance validation: run benchmarks, confirm SC-003 (<16ms) and SC-008 (<5ms) met

---

## Phase 12: Polish & Cross-Cutting Concerns

**Purpose**: Final refinements, code quality, constitution compliance

- [ ] T076 [P] Run cargo clippy: fix all warnings, ensure no pedantic/nursery violations
- [ ] T077 [P] Run cargo fmt: ensure consistent code formatting
- [ ] T078 [P] Review all unwrap() calls: replace with proper error handling or document safety
- [ ] T079 Update constitution check in plan.md: verify all 7 principles still satisfied post-implementation
- [ ] T080 Create feature flag documentation: explain when to enable/disable cache, transition features
- [ ] T081 Add CONTRIBUTING.md: guidelines for future nested routing changes
- [ ] T082 Final review: check all FR-001 through FR-015 from spec.md are implemented

---

## Dependencies & Execution Strategy

### User Story Completion Order

```text
Foundation (Phase 2) → REQUIRED FIRST
    ↓
┌───────────────┬───────────────┐
│  User Story 1 │  User Story 2 │  ← Can run in parallel (independent)
│   (P1 MVP)    │   (P1 MVP)    │
└───────────────┴───────────────┘
    ↓               ↓
    └───────┬───────┘
            ↓
    User Story 3 (P2)  ← Depends on US1 (uses nested resolution)
            ↓
    User Story 4 (P2)  ← Depends on US1 (uses child resolution)
            ↓
    User Story 5 (P3)  ← Depends on US1 (uses param merging)
            ↓
    Error Handling (Phase 8)
            ↓
    Performance (Phase 9)
            ↓
    Documentation (Phase 10)
            ↓
    Testing (Phase 11)
            ↓
    Polish (Phase 12)
```

### Parallel Execution Opportunities

**Phase 2 (Foundation)**: T011-T012 can run in parallel (different test files)

**Phase 3 (US1)**: T013-T014 can run in parallel (different functions in src/nested.rs)

**Phase 4 (US2)**: T021-T022 can run in parallel (cache implementation independent of Route methods)

**Phase 5 (US3)**: T031-T033 can run in parallel (different optimizations, no shared state)

**Phase 6 (US4)**: T037-T038 can run in parallel (validation independent of resolution logic)

**Phase 7 (US5)**: T044-T045 can run in parallel (helper functions, no dependencies)

**Phase 8 (Errors)**: T051-T052 can run in parallel (error boundary vs 404 page, different components)

**Phase 9 (Performance)**: T058-T060 can run in parallel (profiling vs caching vs optimization)

**Phase 10 (Docs)**: T063-T069 ALL can run in parallel (independent documentation files)

**Phase 11 (Testing)**: T070-T073 run sequentially (each validates previous step)

**Phase 12 (Polish)**: T076-T078 can run in parallel (formatting vs linting vs error handling review)

---

## MVP Definition (Minimum Viable Product)

**MVP Scope**: Phase 2 (Foundation) + Phase 3 (US1) + Phase 4 (US2)

**Delivers**:
- ✅ Nested routes with persistent parent layouts
- ✅ Stateful components with LRU caching
- ✅ Basic navigation (push, replace)
- ✅ Unit and integration tests for core functionality

**Does NOT include** (can be added incrementally):
- Deep nesting optimizations (US3)
- Index routes (US4) - can work around with explicit paths
- Parameter inheritance (US5) - can pass params explicitly
- Error boundaries - can add later
- Performance optimizations - sufficient for MVP

**Timeline**: ~2 days (Foundation 1 day, US1+US2 parallel 1 day)

---

## Incremental Delivery Strategy

1. **Week 1**: Foundation + US1 + US2 → MVP Release (v0.2.0-alpha)
2. **Week 2**: US3 + US4 → Beta Release (v0.2.0-beta)
3. **Week 3**: US5 + Error Handling + Performance → RC Release (v0.2.0-rc1)
4. **Week 4**: Documentation + Testing + Polish → Stable Release (v0.2.0)

---

## Task Summary

**Total Tasks**: 82  
**Setup**: 5 tasks  
**Foundation**: 7 tasks (blocking)  
**User Story 1** (P1 MVP): 8 tasks  
**User Story 2** (P1 MVP): 10 tasks  
**User Story 3** (P2): 6 tasks  
**User Story 4** (P2): 7 tasks  
**User Story 5** (P3): 7 tasks  
**Error Handling**: 7 tasks  
**Performance**: 5 tasks  
**Documentation**: 7 tasks  
**Testing**: 6 tasks  
**Polish**: 7 tasks  

**Parallel Opportunities**: 35 tasks marked with [P]  
**Sequential Bottlenecks**: Foundation (Phase 2) must complete first  

**Estimated Timeline**: 
- MVP (US1+US2): 2-3 days
- Full Feature (US1-US5): 5-7 days
- Production Ready (with polish): 8-10 days

---

## Format Validation ✅

- [x] All tasks have checkbox `- [ ]`
- [x] All tasks have sequential ID (T001-T082)
- [x] User story tasks have [Story] label ([US1]-[US5])
- [x] Parallelizable tasks marked with [P]
- [x] All tasks include specific file paths
- [x] Tasks organized by user story for independent implementation
- [x] Independent test criteria provided for each story
- [x] Dependency graph shows completion order
- [x] MVP scope clearly defined
