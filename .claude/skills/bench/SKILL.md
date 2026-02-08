---
name: bench
description: Profile and suggest performance improvements for a module
allowed-tools: Read, Grep, Glob, Bash
argument-hint: "[module_name]"
disable-model-invocation: true
---

Analyze performance characteristics of `$ARGUMENTS` (or the whole project if not specified).

1. Read the source code and identify potential performance bottlenecks:
   - Unnecessary allocations (String::new, Vec::new in hot paths)
   - Excessive cloning where references would work
   - Missing caching opportunities
   - O(n^2) algorithms that could be O(n) or O(n log n)
   - Recursive functions that could be iterative
2. Check if route matching uses LRU cache effectively
3. Review GPUI rendering for unnecessary re-renders
4. Suggest concrete improvements with code examples
5. Prioritize suggestions by expected impact (high/medium/low)
