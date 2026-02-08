//! Logging abstraction layer.
//!
//! Provides macros that dispatch to either the [`log`](https://docs.rs/log)
//! or [`tracing`](https://docs.rs/tracing) crate depending on which feature
//! is enabled. The two features are **mutually exclusive** — enable at most one.
//!
//! | Feature    | Backend         | Default |
//! |------------|-----------------|---------|
//! | `log`      | `log` crate     | yes     |
//! | `tracing`  | `tracing` crate | no      |
//!
//! # Available macros
//!
//! - `trace_log!` — finest-grained diagnostic output.
//! - `debug_log!` — information useful for debugging.
//! - `info_log!` — general informational messages.
//! - `warn_log!` — potentially harmful situations.
//! - `error_log!` — error events that might still allow the app to continue.
//!
//! All macros accept `format!`-style arguments:
//!
//! ```ignore
//! use gpui_navigator::{trace_log, debug_log, info_log, warn_log, error_log};
//!
//! trace_log!("Entering resolve for path '{}'", path);
//! debug_log!("Navigating to route: {}", path);
//! info_log!("Navigation complete");
//! warn_log!("Guard returned unexpected value");
//! error_log!("Failed to resolve route: {}", err);
//! ```

/// Emit a **trace**-level log message.
///
/// Dispatches to `log::trace!` or `tracing::trace!` depending on the
/// enabled feature flag. Accepts `format!`-style arguments.
#[macro_export]
macro_rules! trace_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::trace!($($arg)*);
        #[cfg(feature = "log")]
        ::log::trace!($($arg)*);
    };
}

/// Emit a **debug**-level log message.
///
/// Dispatches to `log::debug!` or `tracing::debug!` depending on the
/// enabled feature flag. Accepts `format!`-style arguments.
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::debug!($($arg)*);
        #[cfg(feature = "log")]
        ::log::debug!($($arg)*);
    };
}

/// Emit an **info**-level log message.
///
/// Dispatches to `log::info!` or `tracing::info!` depending on the
/// enabled feature flag. Accepts `format!`-style arguments.
#[macro_export]
macro_rules! info_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::info!($($arg)*);
        #[cfg(feature = "log")]
        ::log::info!($($arg)*);
    };
}

/// Emit a **warn**-level log message.
///
/// Dispatches to `log::warn!` or `tracing::warn!` depending on the
/// enabled feature flag. Accepts `format!`-style arguments.
#[macro_export]
macro_rules! warn_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::warn!($($arg)*);
        #[cfg(feature = "log")]
        ::log::warn!($($arg)*);
    };
}

/// Emit an **error**-level log message.
///
/// Dispatches to `log::error!` or `tracing::error!` depending on the
/// enabled feature flag. Accepts `format!`-style arguments.
#[macro_export]
macro_rules! error_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        ::tracing::error!($($arg)*);
        #[cfg(feature = "log")]
        ::log::error!($($arg)*);
    };
}
