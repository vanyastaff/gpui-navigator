---
name: gen-test
description: Generate comprehensive tests for a module or function
allowed-tools: Read, Write, Edit, Grep, Glob, Bash
argument-hint: "<module_or_function_name>"
---

Generate comprehensive tests for `$ARGUMENTS`.

1. Find the source code for the specified module/function using Grep and Read
2. Analyze all public functions and their signatures
3. Generate tests covering:
   - Happy path (normal usage)
   - Edge cases (empty input, boundaries, None values)
   - Error cases (invalid input, failure scenarios)
   - Integration with other components where relevant
4. Follow project testing conventions:
   - Test naming: `test_<what>_<scenario>`
   - Place unit tests in `tests/unit/<module>.rs`
   - Use `#[gpui::test]` if GPUI context is needed
   - Use descriptive assertion messages
5. Run the generated tests to make sure they compile and pass
