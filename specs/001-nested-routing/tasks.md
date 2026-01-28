# Tasks: Nested Routing Improvements

**Input**: Design documents from `/specs/001-nested-routing/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, quickstart.md ✅

**Tests**: Tests are REQUIRED per spec (FR-010, FR-011) and constitution Principle VII. This feature uses TDD approach for complex nested routing logic.

**Organization**: Tasks are grouped by user story (P1-P5 priorities) to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1-US5)
- Include exact file paths in descriptions

## Path Conventions

- **Single Rust project**: `src/`, `tests/`, `examples/`, `benchmarks/` at repository root
- This is a library crate, no frontend/backend split

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and test infrastructure

- [x] T001 Review current `src/nested.rs` implementation and identify all functions to modify
- [x] T002 Create `tests/unit/` directory for unit tests
- [x] T003 [P] Create `tests/integration/` directory for integration tests
- [x] T004 [P] Create `benchmarks/` directory for performance tests
- [x] T005 [P] Add test utilities helper in `tests/common/mod.rs` (route fixtures, assertion helpers)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core helpers and validation that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T006 [P] Implement `normalize_path()` helper function in `src/nested.rs` (returns Cow<str>)
- [x] T007 [P] Implement `extract_param_name()` helper function in `src/nested.rs` (strips constraints)
- [x] T008 [P] Add unit tests for `normalize_path()` in `tests/unit/path_normalization_tests.rs` (7 test cases)
- [x] T009 [P] Add unit tests for `extract_param_name()` in `tests/unit/parameter_extraction_tests.rs` (5 test cases)
- [x] T010 Add validation to `Route::children()` in `src/route.rs` to enforce single index route per parent
- [x] T011 Add validation to `Route::children()` in `src/route.rs` to warn about absolute child paths (deprecation)
- [x] T012 Modify `RouterState` struct in `src/state.rs` to add `current_params: RouteParams` field
- [x] T013 Update `RouterState::new()` in `src/state.rs` to initialize `current_params` as empty

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Reliable Nested Route Resolution (Priority: P1) 🎯 MVP

**Goal**: Fix critical bugs in route resolution to make nested routing work reliably without hanging or displaying wrong content

**Independent Test**: Create parent route with 3 child routes, navigate between them, verify correct child renders without errors. Run `cargo run --example nested_demo` successfully.

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T014 [P] [US1] Unit test: Static segment matching in `tests/nested_navigation_tests.rs`
- [x] T015 [P] [US1] Unit test: Parameter segment matching in `tests/nested_navigation_tests.rs`
- [x] T016 [P] [US1] Unit test: Exact parent path prefix check in `tests/nested_navigation_tests.rs`
- [x] T017 [P] [US1] Integration test: Shallow nesting (2 levels) in `tests/nested_navigation_tests.rs`
- [x] T018 [P] [US1] Integration test: Deep nesting (3+ levels) in `tests/nested_navigation_tests.rs`
- [x] T019 [P] [US1] Integration test: Index route navigation in `tests/nested_navigation_tests.rs`

### Implementation for User Story 1

- [x] T020 [US1] Fix BUG-001: Remove double normalization in `src/nested.rs` (use `normalize_path()` helper)
- [x] T021 [US1] Fix remaining path extraction logic in `src/nested.rs` (use `normalize_path()` helper)
- [x] T022 [US1] Replace all ad-hoc `trim_start_matches`/`trim_end_matches` with `normalize_path()` in `src/nested.rs`
- [x] T023 [US1] Update `find_index_route()` in `src/nested.rs` to use `normalize_path()` for child paths
- [x] T024 [US1] Replace `println!` with `trace_log!` in `find_index_route()` in `src/nested.rs`
- [x] T025 [US1] Add warning log when no index route found in `find_index_route()` in `src/nested.rs`
- [x] T026 [US1] Update `build_child_path()` in `src/nested.rs` to use `normalize_path()` helper
- [x] T027 [US1] Run all User Story 1 tests and verify they pass (27 tests passing)
- [ ] T028 [US1] Validate `examples/nested_demo.rs` runs without errors for shallow nesting scenarios

**Checkpoint**: Basic nested routing now works reliably. Index routes render correctly. Path normalization is consistent.

---

## Phase 4: User Story 2 - Correct Parameter Inheritance (Priority: P2)

**Goal**: Extract and merge parameters correctly from parent to child routes for multi-level navigation

**Independent Test**: Create `/workspace/:wid/projects/:pid`, navigate to `/workspace/abc/projects/123`, verify child receives both params `{wid: "abc", pid: "123"}`.

### Tests for User Story 2 ⚠️

- [ ] T029 [P] [US2] Unit test: Single parameter extraction in `tests/unit/parameter_extraction_tests.rs`
- [ ] T030 [P] [US2] Unit test: Parameter constraint stripping (`:id{uuid}` → `id`) in `tests/unit/parameter_extraction_tests.rs`
- [ ] T031 [P] [US2] Unit test: Recursive parameter extraction in `tests/unit/parameter_extraction_tests.rs`
- [ ] T032 [P] [US2] Unit test: Parameter inheritance (parent + child) in `tests/unit/parameter_extraction_tests.rs`
- [ ] T033 [P] [US2] Unit test: Parameter name conflict (child overrides parent) in `tests/unit/parameter_extraction_tests.rs`
- [ ] T034 [P] [US2] Integration test: Parameter inheritance navigation in `tests/integration/nested_navigation_tests.rs`

### Implementation for User Story 2

- [ ] T035 [US2] Fix BUG-003: Use `extract_param_name()` in `resolve_child_route()` in `src/nested.rs` line 120
- [ ] T036 [US2] Fix BUG-002: Add recursive resolution for remaining segments in `resolve_child_route()` in `src/nested.rs` after line 125
- [ ] T037 [US2] Implement recursive call to `resolve_child_route()` when `segments.len() > 1` in `src/nested.rs`
- [ ] T038 [US2] Fix BUG-004: Update `RouterOutlet::render()` in `src/widgets.rs` to retrieve `current_params` from `RouterState`
- [ ] T039 [US2] Fix BUG-004: Pass `parent_params` (not empty) to `resolve_child_route()` in `src/widgets.rs` line 211
- [ ] T040 [US2] Update route matching logic in router to store extracted params in `RouterState.current_params`
- [ ] T041 [US2] Run all User Story 2 tests and verify they pass
- [ ] T042 [US2] Update `examples/nested_demo.rs` to add parameter inheritance example (`/products/:id`)
- [ ] T043 [US2] Validate parameter inheritance works for 3-level nested routes

**Checkpoint**: Parameters flow correctly from parent to child. Deep nesting with params works.

---

## Phase 5: User Story 3 - Path Normalization Consistency (Priority: P2)

**Goal**: Handle various path formats (trailing/leading slashes, empty paths) consistently so navigation is reliable

**Independent Test**: Define routes with mixed formats, navigate using variations (`/dashboard`, `/dashboard/`, `dashboard`), verify all resolve to same route.

### Tests for User Story 3 ⚠️

- [x] T044 [P] [US3] Unit test: Empty path normalization in `tests/unit/path_normalization_tests.rs`
- [x] T045 [P] [US3] Unit test: Root path `/` normalization in `tests/unit/path_normalization_tests.rs`
- [x] T046 [P] [US3] Unit test: Trailing slash handling in `tests/unit/path_normalization_tests.rs`
- [x] T047 [P] [US3] Unit test: Leading slash handling in `tests/unit/path_normalization_tests.rs`
- [x] T048 [P] [US3] Unit test: Multiple consecutive slashes in `tests/unit/path_normalization_tests.rs`
- [x] T049 [P] [US3] Integration test: Path format variations navigate to same route in `tests/integration/nested_navigation_tests.rs`

### Implementation for User Story 3

- [x] T050 [US3] Add explicit check for root path `/` in `resolve_child_route()` in `src/nested.rs`
- [x] T051 [US3] Add explicit check for empty current path in `resolve_child_route()` in `src/nested.rs`
- [x] T052 [US3] Update root path handling in `build_child_path()` to use consistent empty string representation in `src/nested.rs`
- [x] T053 [US3] Document path normalization behavior in module-level rustdoc in `src/nested.rs`
- [x] T054 [US3] Run all User Story 3 tests and verify they pass
- [x] T055 [US3] Test edge cases: `//dashboard` (double slash), `/dashboard/` (trailing), etc.

**Checkpoint**: All path formats work consistently. Edge cases handled gracefully.

---

## Phase 6: User Story 4 - Named Outlet Support (Priority: P3)

**Goal**: Support multiple named outlets (main content + sidebar) for complex layouts with parallel child hierarchies

**Independent Test**: Create parent with named children for outlets "main" and "sidebar", render two RouterOutlets with different names, verify each renders its corresponding child independently.

### Tests for User Story 4 ⚠️

- [x] T056 [P] [US4] Unit test: Named outlet resolution in `tests/unit/named_outlet_tests.rs`
- [x] T057 [P] [US4] Unit test: Missing named outlet returns None in `tests/unit/named_outlet_tests.rs`
- [x] T058 [P] [US4] Unit test: Default outlet vs named outlet in `tests/unit/named_outlet_tests.rs`
- [x] T059 [P] [US4] Integration test: Named outlets render independently in `tests/integration/nested_navigation_tests.rs`

### Implementation for User Story 4

- [x] T060 [US4] Improve error message when named outlet not found in `resolve_child_route()` in `src/nested.rs` line 38
- [x] T061 [US4] Add validation for empty named outlet keys in `Route::named_children()` in `src/route.rs`
- [x] T062 [US4] Run all User Story 4 tests and verify they pass
- [x] T063 [US4] Update `examples/nested_demo.rs` to add named outlet example (sidebar + main content)
- [x] T064 [US4] Validate named outlets work independently (changing one doesn't affect other)

**Checkpoint**: Named outlets work correctly. Multi-panel layouts supported.

---

## Phase 7: User Story 5 - Performance Optimization (Priority: P3)

**Goal**: Optimize route resolution for speed (<1ms) and minimal allocations, especially for complex route trees

**Independent Test**: Benchmark route resolution with 100 routes across 5 nesting levels, verify <1ms resolution time and zero unnecessary allocations for static paths.

### Tests for User Story 5 ⚠️

- [ ] T065 [P] [US5] Unit test: Fast path for single-segment routes in `tests/unit/performance_tests.rs`
- [ ] T066 [P] [US5] Unit test: Static route preference over parameter routes in `tests/unit/performance_tests.rs`
- [ ] T067 [P] [US5] Performance benchmark: Shallow resolution in `benchmarks/nested_resolution.rs`
- [ ] T068 [P] [US5] Performance benchmark: Deep resolution (5 levels) in `benchmarks/nested_resolution.rs`
- [ ] T069 [P] [US5] Performance benchmark: Resolution with caching in `benchmarks/nested_resolution.rs`
- [ ] T070 [P] [US5] Performance benchmark: Parameter extraction overhead in `benchmarks/nested_resolution.rs`

### Implementation for User Story 5

- [ ] T071 [US5] Implement OPT-001: Fast path for single-segment routes (avoid Vec allocation) in `src/nested.rs` line 100
- [ ] T072 [US5] Implement OPT-002: Two-pass matching (static routes before parameters) in `src/nested.rs` lines 109-131
- [ ] T073 [US5] Implement BUG-005: Add LRU caching with `#[cfg(feature = "cache")]` in `src/nested.rs`
- [ ] T074 [US5] Add `ResolutionKey` struct in `src/nested.rs` (parent_path, current_path, outlet_name)
- [ ] T075 [US5] Add thread-local `RESOLUTION_CACHE` with LRU (size 100) in `src/nested.rs`
- [ ] T076 [US5] Add cache lookup at start of `resolve_child_route()` in `src/nested.rs`
- [ ] T077 [US5] Add cache store at end of `resolve_child_route()` in `src/nested.rs`
- [ ] T078 [US5] Run all benchmarks and verify <1ms target met for 50 routes/5 levels
- [ ] T079 [US5] Profile allocations and verify zero unnecessary String allocations with static paths
- [ ] T080 [US5] Test caching behavior: repeated lookups should hit cache

**Checkpoint**: Performance targets met. Route resolution is fast and allocation-efficient.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, examples, and final validation

- [ ] T081 [P] Add rustdoc with examples for `resolve_child_route()` in `src/nested.rs`
- [ ] T082 [P] Add rustdoc with examples for `find_index_route()` in `src/nested.rs`
- [ ] T083 [P] Add rustdoc with examples for `build_child_path()` in `src/nested.rs`
- [ ] T084 [P] Add rustdoc with examples for `normalize_path()` in `src/nested.rs`
- [ ] T085 [P] Add rustdoc with examples for `extract_param_name()` in `src/nested.rs`
- [ ] T086 [P] Add module-level documentation explaining resolution algorithm in `src/nested.rs`
- [ ] T087 [P] Update `README.md` with nested routing performance characteristics and parameter inheritance
- [ ] T088 Update `examples/nested_demo.rs` with all new scenarios (params, deep nesting, named outlets)
- [ ] T089 Run complete test suite: `cargo test --all-features`
- [ ] T090 Run all examples: `cargo run --example nested_demo`
- [ ] T091 Check code coverage: `cargo tarpaulin` (target: 80%+ for src/nested.rs)
- [ ] T092 Run clippy: `cargo clippy --all-features` (zero warnings)
- [ ] T093 Run formatter: `cargo fmt --check`
- [ ] T094 Build documentation: `cargo doc --all-features`
- [ ] T095 Validate quickstart.md checklist (all items pass)
- [ ] T096 Final constitution check: Verify all 7 principles still pass
- [ ] T097 Performance regression check: Re-run benchmarks, compare against baseline

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup (Phase 1) - BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational (Phase 2) - Can start once Phase 2 complete
- **User Story 2 (Phase 4)**: Depends on Foundational (Phase 2) - Can start in parallel with US1
- **User Story 3 (Phase 5)**: Depends on Foundational (Phase 2) - Can start in parallel with US1/US2
- **User Story 4 (Phase 6)**: Depends on Foundational (Phase 2) + US1 (basic resolution must work)
- **User Story 5 (Phase 7)**: Depends on Foundational (Phase 2) + US1-US3 (optimizing existing functionality)
- **Polish (Phase 8)**: Depends on all desired user stories being complete

### User Story Dependencies

```
Phase 1 (Setup)
    ↓
Phase 2 (Foundational) ← CRITICAL BLOCKER
    ↓
    ├─→ Phase 3 (US1: Reliable Resolution) 🎯 MVP [P1]
    │       ↓
    │   Phase 6 (US4: Named Outlets) [P3]
    │
    ├─→ Phase 4 (US2: Parameters) [P2] (parallel with US1)
    │
    ├─→ Phase 5 (US3: Path Normalization) [P2] (parallel with US1/US2)
    │
    └─→ Phase 7 (US5: Performance) [P3] (after US1-US3)
            ↓
        Phase 8 (Polish)
```

**Key Points**:
- US1 is MVP and highest priority
- US2 and US3 can run in parallel after Foundational
- US4 requires US1 to be complete (builds on working resolution)
- US5 optimizes US1-US3 (requires those to work first)

### Within Each User Story

1. **Tests written FIRST** (TDD approach per constitution)
2. Tests MUST FAIL before implementation
3. Helpers/utilities before main logic
4. Core implementation
5. Integration with existing code
6. Validate tests pass
7. Update examples

### Parallel Opportunities

**Phase 1 (Setup)**:
- T002, T003, T004, T005 can all run in parallel (creating directories)

**Phase 2 (Foundational)**:
- T006, T007 can run in parallel (different helper functions)
- T008, T009 can run in parallel (independent test files)
- T010, T011 can run in parallel with T012, T013 (different files: route.rs vs state.rs)

**Phase 3 (US1 Tests)**:
- T014, T015, T016 can run in parallel (same file but independent test functions)
- T017, T018, T019 can run in parallel (same file but independent integration tests)

**Phase 4 (US2 Tests)**:
- T029-T034 can all run in parallel (independent test functions)

**Phase 5 (US3 Tests)**:
- T044-T049 can all run in parallel (independent test functions)

**Phase 6 (US4 Tests)**:
- T056-T059 can all run in parallel (independent test functions)

**Phase 7 (US5 Tests)**:
- T065-T070 can all run in parallel (independent benchmarks and tests)

**Phase 8 (Polish)**:
- T081-T087 can all run in parallel (independent documentation tasks)

---

## Parallel Example: User Story 1

```bash
# Step 1: Launch all tests for US1 in parallel (TDD - write first)
Task T014: "Unit test: Static segment matching"
Task T015: "Unit test: Parameter segment matching"
Task T016: "Unit test: Exact parent path prefix check"
Task T017: "Integration test: Shallow nesting"
Task T018: "Integration test: Deep nesting"
Task T019: "Integration test: Index route navigation"

# Step 2: Verify all tests FAIL (red phase)

# Step 3: Implement fixes sequentially (green phase)
Task T020: "Fix BUG-001: Remove double normalization"
Task T021: "Fix remaining path extraction logic"
Task T022: "Replace ad-hoc normalization"
Task T023: "Update find_index_route normalization"
Task T024: "Replace println with trace_log"
Task T025: "Add warning for missing index route"
Task T026: "Update build_child_path"

# Step 4: Verify all tests PASS (green phase complete)
Task T027: "Run all US1 tests"

# Step 5: Refactor and validate
Task T028: "Validate nested_demo.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only - Fastest Path to Value)

**Goal**: Get reliable nested routing working ASAP

1. Complete Phase 1: Setup (T001-T005) - ~30 min
2. Complete Phase 2: Foundational (T006-T013) - ~2 hours
   - **CRITICAL CHECKPOINT**: Helpers + validation ready
3. Complete Phase 3: User Story 1 (T014-T028) - ~4 hours
   - Write tests first (TDD)
   - Fix critical bugs (BUG-001, path normalization)
   - Validate nested_demo.rs works
4. **STOP and DEMO**: You now have working nested routing!
   - nested_demo.rs runs without errors
   - Dashboard → overview/analytics/settings works
   - Products → product list → detail works
   - Index routes render correctly

**Total Time**: ~6-8 hours for MVP

**Value Delivered**: Core nested routing reliability (addresses P1 blocker)

---

### Incremental Delivery (Add Features Progressively)

**Stage 1: MVP** (Phase 1-3)
- Foundation + reliable resolution
- Demo: nested_demo.rs works perfectly

**Stage 2: Add Parameters** (Phase 4)
- Parameter inheritance across levels
- Demo: /workspace/abc/projects/123 shows both params

**Stage 3: Add Robustness** (Phase 5)
- Path normalization edge cases
- Demo: All path formats work consistently

**Stage 4: Add Advanced Features** (Phase 6-7)
- Named outlets for complex layouts
- Performance optimizations with caching
- Demo: Multi-panel layout + sub-1ms resolution

**Stage 5: Polish** (Phase 8)
- Documentation + examples + final validation
- Ready for release

---

### Parallel Team Strategy

**With 2 developers:**

1. **Together**: Complete Phase 1-2 (Setup + Foundational)
2. **Split**:
   - Dev A: Phase 3 (US1 - Reliable Resolution)
   - Dev B: Phase 4 (US2 - Parameters)
3. **Merge + Demo**: Both stories work independently
4. **Continue**:
   - Dev A: Phase 5 (US3 - Normalization)
   - Dev B: Phase 6 (US4 - Named Outlets)
5. **Together**: Phase 7 (US5 - Performance) + Phase 8 (Polish)

**With 3 developers:**

1. **Together**: Phase 1-2
2. **Split**:
   - Dev A: Phase 3 (US1)
   - Dev B: Phase 4 (US2)
   - Dev C: Phase 5 (US3)
3. **Merge + Demo**: Three independent stories complete
4. **Continue**:
   - Dev A: Phase 6 (US4)
   - Dev B: Phase 7 (US5)
   - Dev C: Phase 8 (Polish - start docs early)

---

## Task Count Summary

- **Phase 1 (Setup)**: 5 tasks
- **Phase 2 (Foundational)**: 8 tasks
- **Phase 3 (US1 - P1 MVP)**: 15 tasks (6 tests + 9 implementation)
- **Phase 4 (US2 - P2)**: 15 tasks (6 tests + 9 implementation)
- **Phase 5 (US3 - P2)**: 12 tasks (6 tests + 6 implementation)
- **Phase 6 (US4 - P3)**: 9 tasks (4 tests + 5 implementation)
- **Phase 7 (US5 - P3)**: 16 tasks (6 tests + 10 implementation)
- **Phase 8 (Polish)**: 17 tasks
- **TOTAL**: 97 tasks

**Parallel Opportunities**: ~40 tasks can run in parallel (marked with [P])

**MVP Scope**: 28 tasks (Phase 1-3) for fastest path to working nested routing

---

## Validation Checklist (Before Marking Feature Complete)

- [ ] All 97 tasks completed
- [ ] All tests pass: `cargo test --all-features`
- [ ] Code coverage ≥80% for src/nested.rs
- [ ] All examples run: `cargo run --example nested_demo`
- [ ] No clippy warnings: `cargo clippy --all-features`
- [ ] Formatted: `cargo fmt --check`
- [ ] Documentation builds: `cargo doc --all-features`
- [ ] Performance <1ms: `cargo bench` (nested_resolution benchmarks)
- [ ] Quickstart validation complete (all checklist items pass)
- [ ] Constitution compliance verified (all 7 principles pass)
- [ ] No regressions in existing functionality

---

## Notes

- **[P] marker**: 40 tasks can run in parallel (different files, no dependencies)
- **[US1-US5] labels**: Map tasks to user stories for traceability
- **TDD approach**: All test tasks (T014-T070) written BEFORE implementation
- **Independent stories**: Each US1-US5 can be tested and demoed independently
- **Checkpoints**: Validate after each phase before moving to next
- **Commit strategy**: Commit after each task or logical group (e.g., all US1 tests together)
- **MVP fast track**: Phase 1-3 only (28 tasks) gets you working nested routing
- **Constitution alignment**: All tasks support Principle IV (Nested Routing Excellence)

**Ready to implement!** Start with Phase 1 (Setup) and proceed sequentially through phases, or split work across team using parallel opportunities.
