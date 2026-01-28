# Implementation Plan: Nested Routing Improvements

**Branch**: `001-nested-routing` | **Date**: 2026-01-28 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-nested-routing/spec.md`

## Summary

Improve nested routing logic in `src/nested.rs` by fixing 6 high-severity bugs, optimizing performance, and adding comprehensive tests. Primary issues include incomplete parameter extraction (recursive parameters lost), double path normalization breaking comparisons, parent parameters not inherited, and no caching causing repeated recomputation. Technical approach: recursive child resolution, consistent path normalization helper, LRU caching (feature flag), and two-pass matching (static routes before parameters). Expected outcome: <1ms route resolution, 80%+ test coverage, zero allocation waste, and fully documented APIs.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: GPUI 0.2.x (GPU-accelerated UI framework), lru 0.16 (optional LRU cache)  
**Storage**: N/A (in-memory route trees using Arc<Route>)
**Testing**: cargo test (unit + integration tests), GPUI test harness for UI tests  
**Target Platform**: Desktop (Windows, macOS, Linux via GPUI runtime)  
**Project Type**: Single Rust library crate with examples  
**Performance Goals**: Route resolution <1ms for 50 routes/5 nesting levels, 60fps transitions  
**Constraints**: No unsafe code (enforced by lints), must work with/without optional features  
**Scale/Scope**: Typical apps have ≤50 routes with ≤5 nesting levels, caching optional for larger trees

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Principle I: API-First Design (NON-NEGOTIABLE)
**Status**: ✅ PASS

- ✅ No breaking API changes - all fixes are internal to src/nested.rs
- ✅ Existing ergonomic APIs preserved (Route::component, Route::view, etc.)
- ✅ Public APIs documented with rustdoc + examples (FR-009)
- ✅ Common use cases remain simple (3-5 lines of code for route definition)

**Justification**: This is a bug fix + optimization pass, not an API redesign. Developer experience improves through reliability, not API changes.

---

### Principle II: React-Router & Go-Router Inspired Architecture
**Status**: ✅ PASS

- ✅ Maintains declarative route definition (no imperative changes)
- ✅ RouterOutlet component pattern preserved
- ✅ Named routes and parameters continue working (with fixes)
- ✅ Nested routing architecture unchanged, only resolution logic improved

**Justification**: Fixes align with React-Router's parameter inheritance and Go-Router's nested route patterns. No architectural drift.

---

### Principle III: Smooth Transitions & Production Polish (NON-NEGOTIABLE)
**Status**: ✅ PASS

- ✅ Transitions unaffected (this work focuses on resolution, not rendering)
- ✅ No changes to default error pages
- ✅ Performance improvements support 60fps target (SC-008)

**Justification**: This feature does not touch transition or UI rendering code. Navigation reliability improvements support smooth UX.

---

### Principle IV: Nested Routing Excellence
**Status**: ✅ PASS (PRIMARY FOCUS)

- ✅ **Fixes parent/child resolution bugs** (BUG-001, BUG-002, BUG-004) - addresses P1 user story
- ✅ **Correct parameter inheritance** (BUG-002, BUG-003, BUG-004) - addresses P2 user story
- ✅ **Path normalization consistency** (ISSUE-007, ISSUE-008) - addresses P2 user story
- ✅ **Named outlet support improved** (BUG-005, ISSUE-051) - addresses P3 user story
- ✅ **Index routes clarified** (BUG-006) - handles edge cases from spec

**Justification**: This feature IS the nested routing excellence initiative. All 20 identified issues directly support Principle IV.

---

### Principle V: Type Safety & Rust Idioms
**Status**: ✅ PASS

- ✅ Uses Arc<Route> for shared route trees (existing, preserved)
- ✅ Adds Cow<str> for path normalization (OPT-001, OPT-003) - zero-cost abstraction
- ✅ No unsafe code (enforced by existing lints)
- ✅ Generic bounds clear (existing RouteBuilder pattern preserved)
- ✅ Idiomatic recursion for nested resolution (BUG-002 fix)

**Justification**: All optimizations leverage Rust idioms (Cow for borrowing, Arc for immutability, LRU cache with RefCell for interior mutability).

---

### Principle VI: Feature Flags & Modularity
**Status**: ✅ PASS

- ✅ Caching is opt-in via existing `cache` feature flag (BUG-005)
- ✅ Core routing works without optional dependencies
- ✅ No new mandatory dependencies added
- ✅ log vs tracing backends remain mutually exclusive

**Justification**: LRU caching (only new performance feature) uses existing `cache` flag. Binary size unchanged for users without caching.

---

### Principle VII: Test-First for Complex Features
**Status**: ✅ PASS

- ✅ Complex nested routing logic REQUIRES tests (user story requirement)
- ✅ Route matching has unit tests (FR-010)
- ✅ Nested navigation has integration tests (FR-011)
- ✅ Target: 80%+ coverage for src/nested.rs (SC-004)
- ✅ Regression tests for each bug fix
- ✅ Examples serve as executable documentation

**Justification**: This is a complex feature (6 HIGH bugs, 9 MEDIUM issues). TDD approach required per constitution for such complexity.

---

### VERDICT: ✅ ALL GATES PASS

No constitution violations. No complexity justifications needed (all changes simplify or fix existing code). Proceed to Phase 0.

---

## Project Structure

### Documentation (this feature)

```text
specs/001-nested-routing/
├── plan.md              # This file
├── research.md          # Bug analysis + solutions (Phase 0 output)
├── data-model.md        # Entity definitions (Phase 1 output)
├── quickstart.md        # Implementation guide (Phase 1 output)
└── checklists/
    └── requirements.md  # Spec quality checklist
```

### Source Code (repository root)

```text
src/
├── nested.rs            # PRIMARY: resolve_child_route, find_index_route, build_child_path
├── route.rs             # MODIFY: Add validation for single index route
├── state.rs             # MODIFY: Store current_params in RouterState
├── widgets.rs           # MODIFY: Pass parent params to resolve_child_route
└── lib.rs               # UNCHANGED

tests/
├── unit/
│   └── nested_resolution_tests.rs  # NEW: Unit tests for resolution logic
└── integration/
    └── nested_navigation_tests.rs  # NEW: Integration tests for navigation flows

examples/
├── nested_demo.rs       # MODIFY: Add parameter inheritance, deep nesting examples
└── ...

benchmarks/
└── nested_resolution.rs # NEW: Performance benchmarks
```

**Structure Decision**: Single project structure (Option 1). This is a library crate, no frontend/backend split. Tests organized by unit (isolated functions) vs integration (full navigation flows). Examples demonstrate real-world usage.

---

## Complexity Tracking

> **No violations found**. All changes simplify existing code or fix bugs. No additional abstractions, patterns, or dependencies beyond existing codebase.

---

## Phase 0: Research & Analysis (COMPLETED)

### Research Findings

**Output**: [research.md](research.md)

**Key Discoveries**:

1. **20 distinct issues identified** across 6 categories:
   - Route Resolution (3 HIGH bugs)
   - Path Normalization (3 MEDIUM issues)
   - Index Routes (2 MEDIUM issues)
   - Parameter Handling (3 HIGH bugs)
   - Named Outlets (2 MEDIUM issues)
   - Performance (4 MEDIUM issues + 1 HIGH)

2. **Critical bugs blocking functionality**:
   - BUG-001: Double normalization breaks path comparison
   - BUG-002: Recursive parameters not extracted (TODO comment confirms incomplete)
   - BUG-003: Parameter constraints not stripped (":id{uuid}" stored as "id{uuid}")
   - BUG-004: Parent parameters lost (RouterOutlet passes empty RouteParams)
   - BUG-005: No caching causes repeated recomputation every render
   - BUG-006: Multiple index routes cause ambiguous behavior

3. **Comparative analysis**:
   - gpui-nav: Simple stack, no nested routing (learnings: stack semantics preserved)
   - gpui-router: React-inspired but basic path matching (learnings: need comprehensive normalization)

4. **Proposed solutions**:
   - Recursive child resolution (BUG-002 fix)
   - Path normalization helper (ISSUE-007, ISSUE-008 fix)
   - LRU caching with feature flag (BUG-005 fix)
   - Two-pass matching (static before parameter) (OPT-002)

---

## Phase 1: Design & Architecture (COMPLETED)

### Data Model

**Output**: [data-model.md](data-model.md)

**Core Entities**:

1. **Route** - Immutable routing configuration
   - Fields: path, builder, children, named_children, name, transition, guard, middleware
   - Invariants: Child paths relative, max one index route per children vec
   - Normalization: Paths stored AS-IS, normalized on-demand

2. **RouteParams** - Parameter map (String → String)
   - Operations: new, get, insert, merge, clone
   - Invariants: No constraint braces in keys, no empty values

3. **ResolvedChildRoute** - (Arc<Route>, RouteParams) tuple
   - Result of successful resolution
   - Short-lived (per render unless cached)

4. **NormalizedPath** - Cow<'a, str> helper
   - Canonical form: leading/trailing slashes removed, root "/" → ""
   - Avoids allocations when already normalized

5. **ResolutionKey** - Cache key (parent_path, current_path, outlet_name)
   - Used with LRU cache (feature flag)
   - Thread-local storage

**Performance Characteristics**:
- Uncached resolution: O(|path| + |segments| + c) where c = children count
- Cached resolution: O(1) hash lookup
- Recursive resolution: O(depth * children_per_level)
- Target: <1ms for depth ≤ 5, children ≤ 20

---

### Implementation Strategy

**Approach**: Incremental fixes, backward compatible

**Phase 1: Critical Bugs** (Non-Breaking)
1. Fix parameter extraction (BUG-002, BUG-003, BUG-004)
2. Fix double normalization (BUG-001)
3. Add path normalization helper (ISSUE-007)

**Phase 2: Optimizations** (Non-Breaking)
1. Add caching (BUG-005, feature flag)
2. Reduce allocations (OPT-001)
3. Prefer static routes (OPT-002)

**Phase 3: Validation** (Breaking only for invalid routes)
1. Enforce single index route (BUG-006)
2. Enforce relative child paths (EDGE-003)

**Phase 4: Testing & Docs**
1. Unit tests (80%+ coverage)
2. Integration tests (5 user stories)
3. Rustdoc + examples

---

### Quickstart Guide

**Output**: [quickstart.md](quickstart.md)

**Step-by-Step**:
1. Fix parameter bugs (recursive extraction, constraint stripping)
2. Fix normalization (helper function, consistent usage)
3. Add caching (LRU with feature flag)
4. Add optimizations (fast path for single segment, two-pass matching)
5. Validate (single index, relative paths)
6. Test (unit + integration + examples)
7. Document (rustdoc + examples)

**Validation Checklist**:
- All tests pass
- Examples run without errors
- Coverage ≥80%
- No clippy warnings
- Performance <1ms

---

## Phase 2: Implementation Tasks (NOT GENERATED YET)

**Note**: Run `/speckit.tasks` to generate detailed task breakdown.

**Expected task structure** (from spec template):
- Phase 1: Setup (project structure, dependencies)
- Phase 2: Foundational (shared infrastructure)
- Phase 3: User Story 1 - Reliable Route Resolution (P1) 🎯 MVP
- Phase 4: User Story 2 - Parameter Inheritance (P2)
- Phase 5: User Story 3 - Path Normalization (P2)
- Phase 6: User Story 4 - Named Outlets (P3)
- Phase 7: User Story 5 - Performance (P3)
- Phase N: Polish & Documentation

Each phase will have:
- Tests written FIRST (TDD per Principle VII)
- Implementation tasks with file paths
- Checkpoints for independent story validation

---

## Risk Assessment

### Technical Risks

| Risk | Impact | Likelihood | Mitigation |
|------|--------|-----------|------------|
| Recursive resolution breaks existing routes | HIGH | LOW | Comprehensive integration tests + examples validation |
| Caching introduces stale data | MEDIUM | LOW | Cache invalidation on navigation context reset + feature flag opt-in |
| Performance regression from normalization overhead | MEDIUM | LOW | Cow<str> avoids allocations + fast path for single segment |
| Breaking changes for invalid routes | LOW | MEDIUM | Deprecation warnings first, reject in v0.2.0 |

### Compatibility Risks

| Risk | Impact | Likelihood | Mitigation |
|------|--------|-----------|------------|
| Users with multiple index routes break | LOW | LOW | Validation logs warning, most users have single index |
| Users with absolute child paths break | LOW | LOW | Trim leading slash preserves behavior, add deprecation warning |
| Cache feature flag breaks downstream | LOW | LOW | Feature is opt-in, no behavior change without flag |

---

## Success Criteria Mapping

From [spec.md](spec.md):

- **SC-001**: All nested_demo.rs scenarios execute without errors
  - **Plan**: Phase 4 integration tests + example validation
  
- **SC-002**: Route resolution <1ms for 50 routes/5 levels
  - **Plan**: Caching (BUG-005) + allocations reduction (OPT-001)
  
- **SC-003**: All 5 user stories have passing integration tests
  - **Plan**: Phase 3-7 implementation + tests per story
  
- **SC-004**: Code coverage 80%+ for src/nested.rs
  - **Plan**: Unit tests (FR-010) + integration tests (FR-011)
  
- **SC-005**: Zero unnecessary String allocations with static paths
  - **Plan**: Cow<str> normalization + fast path optimization
  
- **SC-006**: All public functions have rustdoc with examples
  - **Plan**: Phase N documentation tasks
  
- **SC-007**: Developers create 3-level routes in <10 lines
  - **Plan**: No API changes, ergonomics preserved
  
- **SC-008**: Navigation feels instant (60fps transitions)
  - **Plan**: <1ms resolution + no jank from allocations

---

## Testing Strategy

### Unit Tests (tests/unit/nested_resolution_tests.rs)

**Categories**:
1. Path normalization (7 tests)
2. Segment matching (5 tests)
3. Parameter extraction (6 tests)
4. Index routes (4 tests)
5. Named outlets (3 tests)
6. Performance (2 tests)

**Total**: ~27 unit tests

**Coverage Target**: 80%+ for src/nested.rs

---

### Integration Tests (tests/integration/nested_navigation_tests.rs)

**Scenarios** (from user stories):
1. Shallow nesting (2 levels) - P1
2. Deep nesting (3+ levels) - P1
3. Parameter inheritance - P2
4. Path format variations - P2
5. Named outlets - P3
6. Performance stress test - P3

**Total**: ~15 integration tests

**Target**: All 5 user stories independently testable

---

### Example Validation

**Files to test**:
- examples/nested_demo.rs (existing + new scenarios)

**New scenarios to add**:
- Parameter inheritance (/workspace/:wid/projects/:pid)
- Deep nesting (3+ levels)
- Named outlets (main + sidebar)
- Edge cases (no index route)

---

### Performance Benchmarks

**File**: benchmarks/nested_resolution.rs

**Benchmarks**:
1. resolve_shallow (2 levels, 5 routes)
2. resolve_deep (5 levels, 50 routes)
3. resolve_with_cache (repeated lookups)
4. resolve_with_params (parameter extraction)

**Target**: <1ms for typical case (50 routes, 5 levels)

---

## Documentation Plan

### Public API Docs (Rustdoc)

**Functions to document**:
1. `resolve_child_route` - Primary resolution function
2. `find_index_route` - Index route selection
3. `build_child_path` - Path construction
4. `normalize_path` (internal helper)
5. `extract_param_name` (internal helper)

**Format**: Description + arguments + returns + examples + edge cases

---

### Examples

**Update existing**:
- examples/nested_demo.rs (add scenarios from quickstart)

**Create new** (if needed):
- examples/parameter_inheritance.rs
- examples/named_outlets.rs

---

### README Updates

**Section to update**: Nested Routing section

**Add**:
- Performance characteristics (<1ms resolution)
- Parameter inheritance explanation
- Index route behavior
- Named outlet usage

---

## Deployment Strategy

### Version Bump

**Type**: PATCH (0.1.3 → 0.1.4)

**Rationale**: Bug fixes + performance improvements, no breaking changes

---

### Release Notes

```markdown
## [0.1.4] - 2026-01-28

### Fixed
- Recursive parameter extraction for deeply nested routes (#BUG-002)
- Parameter constraints stripped correctly (e.g., `:id{uuid}` → `id`) (#BUG-003)
- Parent route parameters inherited by child routes (#BUG-004)
- Double path normalization causing comparison failures (#BUG-001)
- Index route ambiguity when multiple index routes defined (#BUG-006)

### Performance
- Added optional LRU caching for route resolution (`cache` feature flag) (#BUG-005)
- Reduced allocations in single-segment path matching (~70% of cases) (#OPT-001)
- Static routes now matched before parameter routes for predictable behavior (#OPT-002)

### Documentation
- Added rustdoc with examples for all nested routing APIs
- Updated nested_demo.rs with parameter inheritance and deep nesting examples

### Deprecated
- Absolute child paths (leading `/`) will be rejected in v0.2.0 (currently warns)
- Multiple index routes per parent will be rejected in v0.2.0 (currently warns)
```

---

### Migration Guide

**For users with multiple index routes**:

Before (works with warning):
```rust
Route::new("/parent", ...)
    .children(vec![
        Route::new("", ...),      // Index 1
        Route::new("index", ...), // Index 2 - WARN
    ])
```

After (recommended):
```rust
Route::new("/parent", ...)
    .children(vec![
        Route::new("", ...),      // Single index
        Route::new("overview", ...), // Renamed
    ])
```

**For users with absolute child paths**:

Before (works with warning):
```rust
Route::new("/parent", ...)
    .children(vec![
        Route::new("/child", ...), // Absolute - WARN
    ])
```

After (recommended):
```rust
Route::new("/parent", ...)
    .children(vec![
        Route::new("child", ...), // Relative
    ])
```

---

## Next Steps

1. **Run `/speckit.tasks`** to generate detailed task breakdown
2. **Implement Phase 1 (Critical Bugs)** first - highest impact
3. **Validate with examples** after each phase
4. **Add tests** before marking tasks complete (TDD)
5. **Review against constitution** before submitting PR

---

## Appendix: File Change Summary

| File | Change Type | Lines Changed (est.) | Complexity |
|------|-------------|---------------------|------------|
| src/nested.rs | MAJOR REFACTOR | ~150 lines | HIGH |
| src/route.rs | MINOR ADDITION | ~30 lines | LOW |
| src/state.rs | MINOR ADDITION | ~10 lines | LOW |
| src/widgets.rs | MINOR CHANGE | ~5 lines | LOW |
| tests/unit/nested_resolution_tests.rs | NEW FILE | ~300 lines | MEDIUM |
| tests/integration/nested_navigation_tests.rs | NEW FILE | ~200 lines | MEDIUM |
| examples/nested_demo.rs | MODIFICATION | ~100 lines added | LOW |
| benchmarks/nested_resolution.rs | NEW FILE | ~100 lines | LOW |

**Total Estimated LOC**: ~895 lines (implementation + tests + docs)

---

## Approval Checkpoint

Before proceeding to `/speckit.tasks`:

- ✅ Constitution check passed (all 7 principles)
- ✅ Research completed (20 issues identified)
- ✅ Data model defined (5 entities, clear relationships)
- ✅ Quickstart guide created (step-by-step implementation)
- ✅ Success criteria mapped to implementation plan
- ✅ No open questions or blockers

**Status**: ✅ READY FOR TASK GENERATION

Run `/speckit.tasks` to proceed.
