# gpui-navigator Development Guidelines

## Project Overview

`gpui-navigator` — библиотека навигации для GPUI (Zed's GPU-accelerated UI framework).
Предоставляет декларативный роутинг с переходами, вложенным роутингом, guards, middleware и LRU-кешированием.

## Technologies

- Rust 1.75+ (edition 2021)
- GPUI 0.2 (Zed's UI framework)
- Optional: lru 0.16, matchit 0.8, log 0.4, tracing 0.1

## Project Structure

```
src/
├── lib.rs          # Library entry point, re-exports
├── route.rs        # Route definition, builder pattern
├── widgets.rs      # RouterOutlet, RouterLink, RouterView components
├── nested.rs       # Nested route resolution, parent-child matching
├── params.rs       # Route parameters, query string parsing
├── state.rs        # RouterState management
├── context.rs      # GPUI integration, navigation API
├── matching.rs     # Segment-based path matching
├── cache.rs        # LRU cache (feature: cache)
├── matcher.rs      # Route pattern matching
├── history.rs      # Navigation history
├── guards.rs       # Route guards (feature: guard)
├── middleware.rs    # Middleware hooks (feature: middleware)
├── transition.rs   # Animations (feature: transition)
├── lifecycle.rs    # Route lifecycle hooks
├── error.rs        # Error handling, custom error pages
└── logging.rs      # Logging abstraction
examples/           # Working demos (nested_demo, transition_demo, etc.)
tests/
├── unit/           # Unit tests (cache, matching, nested, params)
├── integration/    # Integration tests (nested_routing)
└── *.rs            # Top-level test modules
specs/              # Feature specifications and design docs
```

## Commands

```bash
# Build
cargo build --all-features

# Run all tests
cargo test --all-features

# Run specific test module
cargo test --all-features --test integration_tests
cargo test --all-features -- test_name

# Clippy (aggressive lints enabled in Cargo.toml)
cargo clippy --all-targets --all-features

# Format
cargo fmt
cargo fmt --check

# Run example
cargo run --example nested_demo --all-features
cargo run --example transition_demo --all-features

# Doc generation
cargo doc --all-features --no-deps --open
```

## Code Style & Conventions

### Strict Rules

- **unsafe code is FORBIDDEN** — enforced via `[lints.rust] unsafe_code = "forbid"`
- Clippy pedantic + nursery + cargo lints enabled — treat warnings seriously
- All public APIs must have rustdoc documentation
- Builder pattern is the standard for complex type construction (see `Route::new()`)

### Architecture Patterns

- **Route matching**: segment-based matching in `matching.rs`, optional `matchit` for performance
- **State management**: centralized `RouterState` in `state.rs`
- **Component rendering**: `RouterOutlet` renders matched child routes via GPUI's `RenderOnce`
- **Parameter inheritance**: child routes inherit parent parameters through `RouteParams::merge`
- **Feature gates**: guards, middleware, transitions, cache are all behind feature flags

### Testing Requirements

- Run `cargo test --all-features` before any commit
- Test modules mirror source structure: `tests/unit/nested.rs` tests `src/nested.rs`
- Use `#[gpui::test]` for tests requiring GPUI context
- Integration tests go in `tests/integration/`

### Naming Conventions

- Route builder methods: `Route::new("/path").child(...)`, `.component(...)`, `.name("...")`
- Test functions: `test_<what>_<scenario>` (e.g., `test_nested_route_matching_with_params`)
- Feature flags match Cargo.toml features: `#[cfg(feature = "guard")]`

## Features

```toml
default = ["log", "guard", "middleware", "cache", "transition"]
guard = []           # Route guards (AuthGuard, RoleGuard, PermissionGuard)
middleware = []      # Before/after navigation hooks
transition = []      # Fade, slide, scale animations
cache = ["dep:lru"]  # LRU component caching
log = ["dep:log"]    # Log crate backend
tracing = ["dep:tracing"]  # Tracing backend (mutually exclusive with log)
```

## Current Work

Branch `001-nested-routing-redesign` — redesigning nested routing architecture.
- Phases 1-4 complete (296 tests passing)
- Phase 5 pending: deep nesting (4+ levels), index route improvements
- See `specs/001-nested-routing-redesign/` for full design docs

## Important Context

- @Cargo.toml for dependencies and lint configuration
- @specs/001-nested-routing-redesign/spec.md for current feature specification
- @specs/001-nested-routing-redesign/tasks.md for task tracking
