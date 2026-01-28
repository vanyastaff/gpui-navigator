# Implementation Plan: Nested Routing Architecture Redesign

**Branch**: `001-nested-routing-redesign` | **Date**: 2026-01-28 | **Spec**: [spec.md](./spec.md)

## Summary

Redesign nested routing architecture to fix current issues with infinite render loops, stateful component preservation, and hierarchical route resolution. The new design uses segment-based path matching, two-phase rendering (resolve → build), LRU component caching (10 routes default), and hierarchical outlet architecture to prevent recursion. Based on proven patterns from egui_router, React Router, and Yew nested router.

**Key Technical Decisions** (from research.md):
- Segment-based path matching (not regex) using matchit or custom implementation
- Two-phase rendering to prevent double-borrow issues
- Context-based hierarchical architecture (outlets consume path segments)
- LRU cache for stateful components with cancellation-based navigation
- GPUI's dirty tracking for natural loop prevention

---

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)  
**Primary Dependencies**: 
- GPUI 0.2.x (UI framework, already in project)
- matchit 0.8+ (optional, path routing - axum's router)
- lru 0.12+ (optional feature `cache`, LRU cache implementation)

**Storage**: In-memory only (RouterState, ComponentCache)  
**Testing**: 
- cargo test (unit tests)
- Integration tests via examples (nested_demo.rs, etc.)
- Manual testing in GPUI app

**Target Platform**: Desktop (Windows, macOS, Linux via GPUI)  
**Project Type**: Library (gpui-navigator crate)  
**Performance Goals**: 
- Route resolution: <1ms for typical apps (100 routes, 5 levels deep)
- Navigation latency: <16ms (60fps)
- Cache eviction: <5ms when capacity exceeded

**Constraints**: 
- Memory: <100KB cache overhead (10 routes default)
- Render budget: Single frame (16ms) per navigation
- Thread safety: All public APIs Send + Sync
- No unsafe code (Cargo.toml lint enforced)

**Scale/Scope**: 
- Routes: 50-200 typical, up to 1000 supported
- Nesting depth: 5 levels typical, up to 10 supported
- Cache: 10 inactive routes default, configurable

---

## Constitution Check

### ✅ Principle I: API-First Design (NON-NEGOTIABLE)

**Compliance**: ✅ PASS

- Route definition APIs: `Route::new()`, `Route::component()`, `Route::component_with_params()`
- Builder pattern: `.children()`, `.name()`, `.transition()`
- Navigator API: `push()`, `replace()`, `back()`, `forward()`
- Simple usage: Dashboard with 3 children = 6 lines of code

**Evidence**: See contracts/api.md - all APIs require minimal boilerplate, self-documenting

---

### ✅ Principle II: React-Router & Go-Router Inspired

**Compliance**: ✅ PASS

- Declarative route definition (✅)
- RouterOutlet component pattern (✅ inspired by React Router's `<Outlet>`)
- Named routes supported (✅)
- Route parameters with inheritance (✅)
- Nested routing with parent-child hierarchy (✅)

**Evidence**: Architecture directly inspired by React Router's outlet pattern and Yew's nested routers (see research.md)

---

### ✅ Principle III: Smooth Transitions & Production Polish

**Compliance**: ✅ PASS

- Transition support via `#[cfg(feature = "transition")]` (already exists)
- Error boundaries in RouterOutlet (new: displays error UI while parent layout remains)
- Not-found pages (configurable via Router::not_found())
- Graceful degradation (outlet without children renders nothing, no panic)

**Evidence**: FR-014 requires error boundaries, SC-006 requires component errors isolated to outlet

---

### ✅ Principle IV: Nested Routing Excellence

**Compliance**: ✅ PASS - This is the PRIMARY focus of this redesign

- Hierarchical parent/child routes (✅)
- RouterOutlet renders children (✅)
- Named outlets supported (✅ via `RouterOutlet::named()`)
- Parameter inheritance with collision handling (✅ child overrides parent)
- Index routes (empty path "") (✅)
- Path normalization and segment matching (✅)

**Evidence**: All 5 user stories (P1-P3) directly address nested routing scenarios

---

### ✅ Principle V: Type Safety & Rust Idioms

**Compliance**: ✅ PASS

- `Arc<Route>` for immutable shared route tree (✅)
- `Cow<str>` for path operations (✅ recommended in research)
- Builder pattern returns `Self` (✅)
- Generic bounds clear: `F: Fn() -> T where T: Render` (✅)
- No unsafe code (✅ already enforced via lints)

**Evidence**: All data model entities use Arc, no interior mutability in hot paths

---

### ✅ Principle VI: Feature Flags & Modularity

**Compliance**: ✅ PASS

- Core routing works without optional features (✅)
- `cache` feature adds LRU component caching (✅ optional)
- `transition` feature already exists (✅)
- Features documented in README (✅ required by FR-012)

**No violations**: Feature flags properly isolate optional functionality

---

### ✅ Principle VII: Test-First for Complex Features

**Compliance**: ✅ PASS

- Nested routing IS complex → Test-driven required (✅)
- Unit tests for route matching logic (✅ in quickstart.md)
- Integration tests via examples (✅ nested_demo.rs)
- Regression tests for bugs (✅ planned)

**Evidence**: SC-007 requires >80% coverage, SC-008 requires integration tests

---

### 🔄 Post-Design Re-Check (After Phase 1)

**Status**: Phase 1 complete - all design artifacts created

**Changes to Constitution Compliance**: None - all principles remain satisfied

**New Risks Identified**: None - design follows proven patterns from research

**Recommendation**: ✅ Proceed to Phase 2 (tasks generation)

---

## Project Structure

### Documentation (this feature)

```text
specs/001-nested-routing-redesign/
├── spec.md                     # Feature specification (✅ complete)
├── plan.md                     # This file (✅ in progress)
├── research/
│   └── research.md             # Phase 0 research (✅ complete)
├── data-model.md               # Entity relationships, lifecycle (✅ complete)
├── quickstart.md               # Implementation guide with code samples (✅ complete)
├── contracts/
│   └── api.md                  # Public API contracts (✅ complete)
├── checklists/
│   └── requirements.md         # Spec validation checklist (✅ complete)
└── tasks.md                    # Phase 2 - NOT created by /speckit.plan
```

### Source Code (repository root)

```text
gpui-navigator/ (root)
├── src/
│   ├── lib.rs                  # Public API exports
│   ├── route.rs                # Route struct, builder methods
│   ├── state.rs                # RouterState, navigation logic
│   ├── matching.rs             # Path matching, segment-based
│   ├── nested.rs               # Hierarchical route resolution
│   ├── params.rs               # RouteParams map
│   ├── navigator.rs            # Navigator API (push, replace, etc.)
│   ├── widgets.rs              # RouterOutlet, RouterView components
│   ├── cache.rs                # ComponentCache (optional feature)
│   ├── transition.rs           # TransitionConfig (optional feature)
│   └── context.rs              # GlobalRouter wrapper
├── examples/
│   ├── nested_demo.rs          # Main integration test (UPDATE for new arch)
│   ├── stateful_demo.rs        # Stateful component example (UPDATE)
│   └── test_nested.rs          # NEW - integration test for unit validation
├── tests/
│   ├── unit/
│   │   ├── matching.rs         # Path matching tests
│   │   ├── params.rs           # Parameter merging tests
│   │   └── cache.rs            # LRU cache tests (if feature enabled)
│   └── integration/
│       └── nested_routing.rs   # End-to-end nested navigation tests
└── Cargo.toml                  # Update dependencies (matchit?, lru)
```

**Structure Decision**: Single library project with optional features. Core routing in src/, examples serve as integration tests, dedicated tests/ for unit tests. This follows Rust idioms and existing gpui-navigator structure.

---

## Complexity Tracking

> No constitution violations - this section intentionally left empty per template instructions.

---

## Phase 0: Research (✅ COMPLETE)

**Output**: `research/research.md`

**Key Findings**:
1. Segment-based path matching is industry standard (egui_router, React Router, Yew)
2. Two-phase rendering (resolve → build) prevents double-borrow
3. Hierarchical outlet architecture naturally prevents recursion
4. LRU caching with 10-route capacity is reasonable default
5. GPUI's dirty tracking provides loop prevention automatically
6. Navigation cancellation strategy: latest wins

**Decisions Made**:
- Use segment splitting over regex
- Adopt matchit crate for production routing (optional)
- LRU cache default capacity: 10 routes
- Error boundary in outlet, parent layout remains functional
- Cancel in-flight navigation on new navigation start

**Next**: Phase 1 - Design artifacts

---

## Phase 1: Design & Contracts (✅ COMPLETE)

### Artifacts Created:

1. **data-model.md**: 
   - Core entities: Route, RouterState, RouteParams, RouterOutlet, ComponentCache, Navigator
   - Data flow diagrams (navigation, resolution, caching)
   - Lifecycle management (component construction → cache → destruction)
   - Invariants and validation rules

2. **contracts/api.md**:
   - Route definition APIs (new, component, component_with_params, children, name)
   - Navigator methods (push, replace, back, forward, cache management)
   - RouterOutlet component (new, named)
   - RouteParams API (get, set, iter)
   - Error handling contracts
   - Performance guarantees
   - Thread safety commitments

3. **quickstart.md**:
   - Architecture overview diagram
   - Phase-by-phase implementation guide:
     - Phase 1: Core routing (no nesting)
     - Phase 2: Nested routing with hierarchical resolution
     - Phase 3: Component caching (optional feature)
     - Phase 4: Testing strategy
   - Code samples for each phase
   - Common pitfalls & solutions
   - 4-day implementation timeline

### Agent Context Update:

**Command**: `.specify/scripts/powershell/update-agent-context.ps1 -AgentType claude`

**Technologies Added**:
- matchit 0.8+ (optional dependency for path routing)
- lru 0.12+ (optional feature `cache`)
- Segment-based path matching pattern
- Two-phase rendering pattern
- LRU component caching strategy

**Note**: Manual additions preserved between markers per script behavior

---

## Phase 2: Task Decomposition (NOT PART OF /speckit.plan)

**Status**: Phase 2 is handled by `/speckit.tasks` command, not by `/speckit.plan`

**What happens next**:
1. User reviews Phase 0 & Phase 1 artifacts
2. User runs `/speckit.tasks` to generate tasks.md
3. Tasks command reads spec.md, plan.md, research.md, data-model.md, quickstart.md
4. Tasks command generates dependency-ordered implementation tasks

**This plan command STOPS here** as per command specification.

---

## Implementation Phases (Reference Only - NOT Tasks)

### Phase 1: Core Routing (No Nesting) - ~1 day

**Scope**: Single-level routes, path matching, basic navigation

**Key Components**:
- Route struct with builder pattern
- RouterState with navigation method
- Path matching (segment-based)
- Navigator API (push, replace)
- Simple RouterOutlet (no nesting)

**Deliverable**: Can navigate between top-level routes

---

### Phase 2: Nested Routing - ~1 day

**Scope**: Parent-child hierarchies, outlet resolution, parameter inheritance

**Key Components**:
- Hierarchical route resolution (resolve_child_route)
- Index route handling
- Parameter merging
- RouterOutlet with nested resolution
- Parent route detection

**Deliverable**: Can navigate through nested hierarchies (dashboard → overview/analytics)

---

### Phase 3: Component Caching (Optional) - ~0.5 days

**Scope**: LRU cache, stateful component preservation

**Key Components**:
- ComponentCache struct (LRU)
- Route::component() with use_keyed_state
- Cache cleanup API
- Eviction on overflow

**Deliverable**: Components preserve state across navigation

---

### Phase 4: Testing & Validation - ~0.5 days

**Scope**: Unit tests, integration tests, examples

**Key Components**:
- Unit tests for matching, params, cache
- Integration tests via examples
- Update nested_demo.rs
- Performance validation

**Deliverable**: >80% coverage, all user stories pass

---

### Phase 5: Documentation & Polish - ~0.5 days

**Scope**: README updates, rustdoc, error messages

**Key Components**:
- README with quickstart
- Rustdoc for all public APIs
- Clear error messages (FR-012)
- Migration guide from old arch

**Deliverable**: Production-ready documentation

---

**Total Estimated**: ~4 days for full implementation

---

## Testing Strategy

### Unit Tests (src + tests/unit/)

**Coverage Target**: >80% per SC-007

**Test Suites**:
1. **Matching Tests** (`tests/unit/matching.rs`):
   - Exact path matches
   - Parameter extraction
   - No match scenarios
   - Edge cases (empty path, trailing slash)

2. **Parameter Tests** (`tests/unit/params.rs`):
   - Parameter merging (parent + child)
   - Collision handling (child overrides)
   - Empty params
   - Multiple params

3. **Cache Tests** (`tests/unit/cache.rs` - if feature enabled):
   - LRU insertion
   - Eviction on overflow
   - Cache hit/miss
   - Explicit removal

4. **Route Resolution Tests** (`tests/unit/nested.rs`):
   - Single-level resolution
   - Multi-level hierarchies
   - Index route selection
   - Not-found scenarios

### Integration Tests (tests/integration/ + examples/)

**Coverage Target**: All user stories (P1-P3) from spec.md

**Test Scenarios**:
1. **Simple Nested Routes** (User Story 1):
   - Dashboard with sidebar + 3 children
   - Navigate between siblings (layout persists)
   - Parameter changes (layout persists, child updates)

2. **Stateful Components** (User Story 2):
   - Counter page increments, navigate away, return
   - State preserved within cache limit
   - Cache eviction after 10+ routes
   - Explicit cache cleanup

3. **Deep Hierarchies** (User Story 3):
   - 4-level nesting (root → workspace → project → task)
   - No infinite loops
   - Efficient rendering (no tree re-render)

4. **Index Routes** (User Story 4):
   - Parent path shows default child
   - Root index at `/`
   - Multi-level index routes

5. **Parameter Inheritance** (User Story 5):
   - Parent params accessible in child
   - Multi-level param merging
   - Collision handling

### Performance Validation

**Metrics from SC-003, SC-008**:
- Navigation latency: <16ms (measure with Instant::now())
- Cache eviction: <5ms (measure eviction function)
- Route resolution: <1ms for 100 routes, 5 levels deep

**Tools**:
- `cargo bench` for micro-benchmarks
- GPUI profiler for frame timings
- Manual timing with debug logs

---

## Migration Strategy (From Old Architecture)

### Breaking Changes

1. **RouterOutlet usage**: 
   - Old: `RouterOutlet::new()` created new instance every render
   - New: Use `cx.new(|_| RouterOutlet::new())` for cached entity

2. **Route definition**:
   - Old: Implicit nested resolution
   - New: Explicit `resolve_child_route()` in outlet

3. **Component caching**:
   - Old: Manual `use_keyed_state` in every route
   - New: `Route::component()` handles caching automatically

### Migration Steps

1. Update `Cargo.toml` dependencies (matchit, lru if needed)
2. Replace `src/nested.rs` with new implementation
3. Update `src/widgets.rs` (RouterOutlet render logic)
4. Update examples to use new patterns
5. Run tests to verify behavior
6. Update documentation

### Backwards Compatibility

**None**: This is a redesign, not an incremental change. Version bump: 0.1.x → 0.2.0 (MINOR).

**Justification**: Current architecture has fundamental issues (infinite loops, state loss). Clean break enables proper fix.

---

## Risks & Mitigations

### Risk 1: Performance Regression

**Probability**: Low  
**Impact**: High  

**Mitigation**:
- Benchmark before/after with same examples
- Target: <16ms navigation (same as current)
- Use matchit for O(log n) vs O(n) matching if needed

---

### Risk 2: Cache Eviction UX Issues

**Probability**: Medium  
**Impact**: Medium

**Mitigation**:
- Default 10 routes covers 90% of navigation patterns
- Document cache behavior clearly
- Provide explicit cleanup API for edge cases

---

### Risk 3: Breaking Changes Impact

**Probability**: High (expected)  
**Impact**: Medium

**Mitigation**:
- Clear migration guide in CHANGELOG
- Update all examples in PR
- Version bump signals breaking change (0.1 → 0.2)

---

## Success Criteria (from spec.md)

- [Phase 1] **SC-001**: Nested routes 5+ levels deep without errors
- [Phase 2] **SC-002**: Stateful components maintain state
- [Phase 3] **SC-003**: Navigation <16ms
- [Phase 1] **SC-004**: Circular dependency detection
- [Phase 2] **SC-005**: Correct hierarchical render order 100%
- [Phase 4] **SC-006**: Error boundaries isolate failures
- [Phase 4] **SC-007**: >80% test coverage
- [Phase 4] **SC-008**: Integration tests pass
- [Phase 5] **SC-009**: Documentation with examples
- [Phase 1-2] **SC-010**: Unit tests for core scenarios
- [Phase 4] **SC-011**: All user stories validated

---

## Next Steps

**Current Status**: ✅ Phase 0 & Phase 1 COMPLETE

**Artifacts Delivered**:
- [x] research/research.md (8 key findings, 8 architecture decisions)
- [x] data-model.md (6 core entities, flow diagrams, invariants)
- [x] contracts/api.md (Complete public API specification)
- [x] quickstart.md (Phase-by-phase implementation guide)
- [x] plan.md (This file - constitution check, structure, strategy)

**Next Command**: 

```bash
/speckit.tasks
```

**What `/speckit.tasks` will do**:
1. Read all Phase 0 & 1 artifacts
2. Generate dependency-ordered implementation tasks
3. Create tasks.md with concrete work items
4. Each task maps to quickstart phases + testing requirements

**Estimated Timeline**: ~4 days implementation after tasks generated

---

## References

- **Spec**: [spec.md](./spec.md)
- **Research**: [research/research.md](./research/research.md)
- **Data Model**: [data-model.md](./data-model.md)
- **API Contracts**: [contracts/api.md](./contracts/api.md)
- **Quickstart**: [quickstart.md](./quickstart.md)
- **Constitution**: [../../.specify/memory/constitution.md](../../.specify/memory/constitution.md)
