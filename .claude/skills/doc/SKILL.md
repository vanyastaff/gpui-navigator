---
name: doc
description: Generate or improve rustdoc documentation for a module
allowed-tools: Read, Edit, Grep, Glob, Bash
argument-hint: "<module_name>"
---

Improve rustdoc documentation for `$ARGUMENTS`.

1. Find and read the source file for the specified module
2. Identify all public items (`pub fn`, `pub struct`, `pub enum`, `pub trait`)
3. For each public item without documentation:
   - Add `///` doc comments explaining purpose and behavior
   - Include usage examples where helpful
   - Document parameters, return values, and panics
4. For items with existing docs, improve clarity if needed
5. Run `cargo doc --all-features --no-deps` to verify documentation builds
6. Run `cargo test --doc --all-features` to verify doc examples compile
