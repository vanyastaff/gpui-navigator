---
paths:
  - "src/**/*.rs"
  - "examples/**/*.rs"
---

# Rust Code Style Rules

## Safety

- `unsafe` code is forbidden — the project enforces `unsafe_code = "forbid"`
- Never use `.unwrap()` in library code — use `Result` or `Option` combinators
- `.unwrap()` is acceptable only in tests and examples

## Patterns

- Use builder pattern for constructing complex types (see `Route::new()`)
- Feature-gated code uses `#[cfg(feature = "...")]` attributes
- Public API types implement `Debug`, `Clone` where sensible
- Prefer `impl Into<SharedString>` for string parameters in public APIs

## GPUI Integration

- Components implement `RenderOnce` trait for rendering
- Use `cx: &mut WindowContext` for GPUI context access
- Router state is stored as GPUI Global via `cx.set_global()`
- Event handling uses GPUI's `cx.listener()` pattern

## Error Handling

- Custom errors in `src/error.rs`
- Route guards return `bool` — true to allow, false to block
- Navigation errors are logged, not panicked

## Documentation

- All `pub` items must have `///` doc comments
- Include usage examples in doc comments for key APIs
- Use `#[doc(hidden)]` for internal-but-public items
