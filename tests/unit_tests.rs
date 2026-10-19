//! Unit tests runner
//!
//! Entry point for unit tests organized in the tests/unit/ directory.

// Alias the selected GPUI backend to `gpui`, as the library does.
#[cfg(all(
    feature = "gpui-fast",
    not(feature = "gpui-02"),
    not(feature = "gpui-pre")
))]
extern crate gpui_fast as gpui;
#[cfg(all(feature = "gpui-pre", not(feature = "gpui-02")))]
extern crate gpui_pre as gpui;

mod common;
mod unit;
