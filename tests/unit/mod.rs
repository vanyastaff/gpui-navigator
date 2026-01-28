//! Unit tests module
//!
//! Contains unit tests for individual functions and components.

// Legacy tests
mod named_outlet_tests;
mod parameter_extraction_tests;
mod path_normalization_tests;

// New tests for nested routing redesign (Phase 1: T004)
mod cache; // T029 - LRU component cache
mod matching; // T011 - segment-based path matching
mod nested;
mod params; // T012 - RouteParams merging // T019, T042 - hierarchical resolution
