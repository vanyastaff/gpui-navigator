//! Integration tests for nested routing - Phase 4 (T030)

#![allow(
    clippy::future_not_send,
    clippy::unused_async,
    clippy::needless_pass_by_ref_mut
)]

// Alias the selected GPUI backend to `gpui`, as the library does.
#[cfg(all(
    feature = "gpui-fast",
    not(feature = "gpui-02"),
    not(feature = "gpui-pre")
))]
extern crate gpui_fast as gpui;
#[cfg(all(feature = "gpui-pre", not(feature = "gpui-02")))]
extern crate gpui_pre as gpui;

mod integration {
    pub mod nested_routing;
}
