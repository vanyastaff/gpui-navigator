<!--
Sync Impact Report - Constitution v1.0.0

VERSION CHANGE: Initial constitution (0.0.0 → 1.0.0)

MODIFIED PRINCIPLES: N/A (initial version)

ADDED SECTIONS:
- Core Principles (I-VII)
- Technical Standards
- Development Workflow
- Governance

REMOVED SECTIONS: N/A

TEMPLATES STATUS:
- ✅ plan-template.md: Reviewed, no updates needed (constitution check section already compatible)
- ✅ spec-template.md: Reviewed, no updates needed (aligns with API-first principle)
- ✅ tasks-template.md: Reviewed, no updates needed (supports TDD optional approach)

FOLLOW-UP TODOS: None
-->

# GPUI Navigator Constitution

## Core Principles

### I. API-First Design (NON-NEGOTIABLE)

The library MUST prioritize developer experience through clean, intuitive APIs that feel natural and require minimal boilerplate.

**Rules:**
- Every feature MUST have a simple, ergonomic API (Route::view(), Route::component(), Route::component_with_params())
- Builder patterns MUST be chainable and self-documenting (e.g., `.transition()`, `.name()`, `.children()`)
- Common use cases MUST require less code than edge cases
- API design MUST be validated with real-world examples before implementation

**Rationale:** React-Router and Go-Router (Flutter) succeeded because they made routing feel effortless. GPUI Navigator differentiates itself through "zero boilerplate" - a route should be definable in 3-5 lines of clear code, not 20 lines of builders and configuration.

### II. React-Router & Go-Router Inspired Architecture

The library is architecturally inspired by React-Router and Go-Router (Flutter), adapting their proven patterns to Rust and GPUI.

**Rules:**
- Declarative route definition (not imperative navigation state machines)
- Nested routing with RouterOutlet components (React-Router's `<Outlet>`)
- Named routes and route parameters (Go-Router's route naming)
- Route guards and middleware patterns (adapted to Rust's type system)
- Component-based architecture where routes render GPUI components

**Rationale:** These libraries have proven architectures with millions of users. Rather than reinvent routing, we adapt their successful patterns to Rust's ownership model and GPUI's reactive framework, while adding Rust-specific improvements (type safety, zero-cost abstractions).

### III. Smooth Transitions & Production Polish (NON-NEGOTIABLE)

The library MUST provide smooth, professional animations and beautiful default UI out of the box.

**Rules:**
- MUST support fade, slide (left/right/up/down), and custom transitions
- Default error pages (404, loading, errors) MUST be pre-styled and production-ready
- Transitions MUST use dual-animation system (enter + exit animations)
- Animation timing MUST feel smooth (200-400ms defaults, user-configurable)
- NO placeholders or "TODO" pages in production features

**Rationale:** Unlike gpui-nav (lightweight stack) and gpui-router (React-inspired but basic), GPUI Navigator differentiates through production-ready polish. Users should be able to ship apps with beautiful navigation without writing custom error pages or animations.

### IV. Nested Routing Excellence

Nested routing MUST work reliably with parent/child route hierarchies, multiple outlet support, and proper parameter inheritance.

**Rules:**
- Parent routes MUST support multiple child routes
- RouterOutlet MUST render child routes within parent layouts
- Named outlets MUST allow multiple parallel child route hierarchies (e.g., main content + sidebar)
- Route parameters MUST merge from parent to child correctly
- Index routes (default children) MUST work when no child is explicitly selected
- Child route resolution MUST handle path normalization and segment matching correctly

**Rationale:** Nested routing is the primary technical challenge and differentiator. The current codebase has working examples (nested_demo.rs) but edge cases exist. This principle ensures nested routing receives continuous priority and testing.

### V. Type Safety & Rust Idioms

The library MUST leverage Rust's type system for safety while remaining ergonomic.

**Rules:**
- Use `Arc<Route>` for shared route trees (immutable, thread-safe)
- Prefer `Cow<str>` over `String` when values can be borrowed
- Route builders MUST return `Self` for chaining
- Generic parameters MUST use clear bounds (e.g., `F: Fn() -> T where T: Render`)
- Unsafe code is FORBIDDEN (as per Cargo.toml lints)

**Rationale:** Rust enables zero-cost abstractions and compile-time safety. GPUI Navigator should feel like idiomatic Rust code, not a port of JavaScript patterns with .clone() everywhere.

### VI. Feature Flags & Modularity

Optional features (guards, middleware, cache, tracing) MUST be behind feature flags and NOT impose costs on users who don't need them.

**Rules:**
- Core routing MUST work with zero optional dependencies
- Feature flags MUST be documented clearly in README
- Each optional feature MUST justify its inclusion (use case + dependency cost)
- Features MUST NOT break core functionality when disabled
- Logging backends (log vs tracing) MUST be mutually exclusive

**Rationale:** Many users only need basic routing. Heavy features (LRU caching, middleware hooks) should be opt-in. This keeps compile times fast and binary sizes small for simple use cases.

### VII. Test-First for Complex Features

Complex features (nested routing, route resolution, transitions) MUST be test-driven, but simple features MAY skip TDD if complexity doesn't warrant it.

**Rules:**
- Nested routing changes MUST have integration tests
- Route matching logic MUST have unit tests
- Examples (like nested_demo.rs) serve as executable documentation and manual tests
- Regression fixes MUST add tests reproducing the bug
- Simple additions (documentation, styling tweaks) do NOT require upfront tests

**Rationale:** TDD prevents regressions in complex routing logic, but enforcing it for all changes (docs, examples, trivial features) creates friction. Use judgment: if it's complex or has broken before, test it.

## Technical Standards

### Language & Dependencies
- **Rust Version**: 1.75+ (2021 edition)
- **Primary Framework**: GPUI 0.2.x
- **Optional Dependencies**: lru (cache), log/tracing (logging)
- **Linting**: Clippy pedantic + nursery (with pragmatic allows as documented in Cargo.toml)

### Architecture Patterns
- **State Management**: GPUI's reactive Context and Entity system
- **Route Storage**: Global RouterState (Arc-wrapped, managed via App context)
- **Component Lifecycle**: GPUI's Render trait + use_keyed_state for caching
- **Thread Safety**: Immutable Arc<Route> trees, no interior mutability in hot paths

### Performance Goals
- Route resolution: <1ms for typical nested hierarchies (≤5 levels, ≤50 total routes)
- Transitions: 60fps animations (16ms frame budget)
- Memory: Cached components use Entity keyed state (managed by GPUI)

### Code Quality Standards
- All public APIs MUST have rustdoc comments with examples
- Examples MUST run via `cargo run --example <name>`
- Clippy warnings MUST be addressed or explicitly allowed with justification
- `cargo fmt` MUST pass before commits

## Development Workflow

### Feature Development Process
1. **Specification**: Create spec.md with user scenarios and acceptance criteria
2. **Planning**: Write plan.md with technical approach and constitution check
3. **Research** (if needed): Document findings in research.md
4. **Implementation**: Follow tasks.md or work incrementally if tasks not needed
5. **Examples**: Add or update examples demonstrating the feature
6. **Documentation**: Update README.md and rustdoc comments
7. **Review**: Self-review against constitution principles

### Nested Routing Development (Special Focus)
Given current nested routing issues:
- Changes to src/nested.rs MUST be validated with nested_demo.rs
- Add specific test cases for edge cases (index routes, parameter inheritance, deep nesting)
- Log trace statements are acceptable during debugging (use trace_log! macro)
- Study gpui-nav and gpui-router implementations for comparative insights

### Commit Practices
- Commits should be atomic (one logical change)
- Commit messages: `type: description` (e.g., "feat: Add named outlets", "fix: Nested parameter merging")
- Types: feat, fix, docs, refactor, test, chore

### Breaking Changes
- MAJOR version: Breaking API changes (e.g., Route::new signature change)
- MINOR version: New features (e.g., named outlets, new transition types)
- PATCH version: Bug fixes, docs, internal refactors

## Governance

### Constitution Authority
This constitution supersedes informal conventions. When in doubt, consult these principles.

### Amendment Process
1. Propose amendment with rationale in constitution.md comment
2. Discuss impact on existing codebase and templates
3. Update version (MAJOR if changing core principle, MINOR if adding, PATCH if clarifying)
4. Propagate changes to affected templates
5. Document change in Sync Impact Report comment

### Compliance Review
- Every feature implementation MUST pass constitution check (in plan.md)
- Pull requests SHOULD reference which principles guide the design
- Complexity (e.g., additional abstractions, dependencies) MUST be justified against Principle VI (modularity)

### Versioning Policy
Constitution follows semantic versioning:
- **MAJOR**: Backward incompatible governance/principle changes (e.g., removing API-First principle)
- **MINOR**: New principles or sections added (e.g., adding Security principle)
- **PATCH**: Clarifications, wording improvements, typo fixes

**Version**: 1.0.0 | **Ratified**: 2026-01-28 | **Last Amended**: 2026-01-28
