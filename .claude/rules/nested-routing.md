---
paths:
  - "src/nested.rs"
  - "src/matching.rs"
  - "src/widgets.rs"
  - "src/route.rs"
  - "src/state.rs"
  - "src/params.rs"
---

# Nested Routing Architecture Rules

## Core Concepts

- Routes form a tree: parent routes contain child routes via `.child()`
- `RouterOutlet` renders the matched child route at the current level
- Parameters are inherited: child routes receive merged parent + own params
- Index routes (path "") serve as default content for a parent

## Key Invariants

- A parent route with children MUST use `RouterOutlet` in its component to render children
- Route matching is segment-based: "/users/123/posts" matches segments ["users", ":id", "posts"]
- Parameter extraction happens during matching — `RouteParams` is populated before rendering
- Navigation state must be consistent: `RouterState.current_path` always reflects the rendered route

## Performance

- Route resolution uses LRU cache when `cache` feature is enabled
- Avoid deep recursion in route tree traversal — use iterative approaches
- Minimize allocations during matching — reuse `Vec<&str>` for segments

## Common Pitfalls

- Render loops: ensure `RouterOutlet` doesn't trigger re-navigation
- Stale params: always merge fresh params from current match, don't cache params
- Missing outlet: parent without `RouterOutlet` silently swallows child content
