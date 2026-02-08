//! Route transition animations.
//!
//! This module provides a declarative transition system for route changes,
//! gated behind the `transition` feature flag.
//!
//! # Available transitions
//!
//! | Variant | Constructor | Description |
//! |---------|-------------|-------------|
//! | [`Transition::None`] | default | No animation |
//! | [`Transition::Fade`] | [`Transition::fade`] | Cross-fade (old fades out, new fades in) |
//! | [`Transition::Slide`] | [`Transition::slide_left`], etc. | Positional slide in any direction |
//!
//! Each transition carries a `duration_ms` controlling animation length.
//!
//! # Per-route configuration
//!
//! Attach a transition to a route via the builder:
//!
//! ```ignore
//! use gpui_navigator::{Route, Transition};
//!
//! Route::new("/dashboard", |_, cx, _| gpui::div())
//!     .transition(Transition::fade(200));
//! ```
//!
//! # One-off overrides
//!
//! Use [`TransitionConfig::set_override`] or `Navigator::push_with_transition`
//! to override the default for a single navigation.

use gpui::{div, px, Div, IntoElement, ParentElement, Styled};
use std::time::Duration;

/// Direction for slide transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SlideDirection {
    /// Slide from left to right
    Left,
    /// Slide from right to left
    Right,
    /// Slide from top to bottom
    Up,
    /// Slide from bottom to top
    Down,
}

/// Built-in transition types for route animations.
///
/// # Examples
///
/// ```
/// use gpui_navigator::transition::Transition;
/// use std::time::Duration;
///
/// let fade = Transition::fade(200);
/// assert_eq!(fade.duration(), Duration::from_millis(200));
/// assert!(!fade.is_none());
///
/// let slide = Transition::slide_left(300);
/// assert_eq!(slide.duration(), Duration::from_millis(300));
/// ```
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub enum Transition {
    /// No transition animation
    #[default]
    None,

    /// Cross-fade transition: old content fades out while new content fades in
    Fade {
        /// Duration in milliseconds
        duration_ms: u64,
    },

    /// Slide transition
    Slide {
        /// Direction to slide
        direction: SlideDirection,
        /// Duration in milliseconds
        duration_ms: u64,
    },
}

impl Transition {
    /// Create a cross-fade transition (old fades out, new fades in simultaneously)
    #[must_use] 
    pub const fn fade(duration_ms: u64) -> Self {
        Self::Fade { duration_ms }
    }

    /// Create a slide-left transition
    #[must_use] 
    pub const fn slide_left(duration_ms: u64) -> Self {
        Self::Slide {
            direction: SlideDirection::Left,
            duration_ms,
        }
    }

    /// Create a slide-right transition
    #[must_use] 
    pub const fn slide_right(duration_ms: u64) -> Self {
        Self::Slide {
            direction: SlideDirection::Right,
            duration_ms,
        }
    }

    /// Create a slide-up transition
    #[must_use] 
    pub const fn slide_up(duration_ms: u64) -> Self {
        Self::Slide {
            direction: SlideDirection::Up,
            duration_ms,
        }
    }

    /// Create a slide-down transition
    #[must_use] 
    pub const fn slide_down(duration_ms: u64) -> Self {
        Self::Slide {
            direction: SlideDirection::Down,
            duration_ms,
        }
    }

    /// Get the duration of this transition
    #[must_use] 
    pub const fn duration(&self) -> Duration {
        match self {
            Self::None => Duration::ZERO,
            Self::Fade { duration_ms, .. } | Self::Slide { duration_ms, .. } => {
                Duration::from_millis(*duration_ms)
            }
        }
    }

    /// Check if this is a no-op transition
    #[must_use] 
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Per-route transition configuration with optional one-off override.
///
/// Stores a `default` transition applied to every navigation and an optional
/// `override_next` that takes precedence for the next navigation only.
#[derive(Clone)]
pub struct TransitionConfig {
    /// Default transition for this route
    pub default: Transition,

    /// Override transition for specific navigation
    pub override_next: Option<Transition>,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            default: Transition::None,
            override_next: None,
        }
    }
}

impl TransitionConfig {
    /// Create a new transition config with a default transition
    #[must_use] 
    pub const fn new(default: Transition) -> Self {
        Self {
            default,
            override_next: None,
        }
    }

    /// Get the active transition (override if set, otherwise default)
    #[must_use] 
    pub fn active(&self) -> &Transition {
        self.override_next.as_ref().unwrap_or(&self.default)
    }

    /// Set an override transition for the next navigation
    pub fn set_override(&mut self, transition: Transition) {
        self.override_next = Some(transition);
    }

    /// Clear the override transition
    pub fn clear_override(&mut self) {
        self.override_next = None;
    }

    /// Check if there's an active override
    #[must_use] 
    pub const fn has_override(&self) -> bool {
        self.override_next.is_some()
    }
}

// ============================================================================
// Transition Builder
// ============================================================================

/// Context values passed to custom transition renderers.
pub struct TransitionContext {
    /// Animation progress from 0.0 to 1.0
    pub animation: f32,
    /// Secondary animation for exit transitions (1.0 to 0.0)
    pub secondary_animation: f32,
}

/// Apply a transition effect to an element.
///
/// Given a `progress` value in `0.0..=1.0`, wraps `element` in a [`Div`]
/// with the appropriate positional offset and opacity for the transition type.
///
/// - [`Transition::None`] — returns the element unchanged (opacity 1, no offset).
/// - [`Transition::Fade`] — sets opacity to `progress`.
/// - [`Transition::Slide`] — offsets by `(1 - progress) * 100px` in the
///   appropriate direction while also fading in.
pub fn apply_transition(element: impl IntoElement, transition: &Transition, progress: f32) -> Div {
    // Always use consistent method chain to avoid recursion limit
    // Calculate all values first, then apply them in one chain
    let (x, y, opacity) = match transition {
        Transition::None => (0.0, 0.0, 1.0),

        Transition::Fade { .. } => {
            // Fade effect — progress controls opacity
            (0.0, 0.0, progress)
        }

        Transition::Slide { direction, .. } => {
            let offset_px = (1.0 - progress) * 100.0;
            let (x, y) = match direction {
                SlideDirection::Left => (offset_px, 0.0),
                SlideDirection::Right => (-offset_px, 0.0),
                SlideDirection::Up => (0.0, offset_px),
                SlideDirection::Down => (0.0, -offset_px),
            };
            (x, y, progress)
        }
    };

    // Unified return type - same method chain for all branches
    div()
        .relative()
        .left(px(x))
        .top(px(y))
        .opacity(opacity)
        .child(element)
}

/// Cubic ease-in-out easing function (`t` in `0.0..=1.0`).
#[must_use] 
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0f32).mul_add(t, 2.0).powi(3) / 2.0
    }
}

/// Clamp `progress` to `0.0..=1.0` and apply [`ease_in_out_cubic`].
#[must_use] 
pub fn apply_easing(progress: f32) -> f32 {
    ease_in_out_cubic(progress.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide_direction() {
        assert_eq!(SlideDirection::Left, SlideDirection::Left);
        assert_ne!(SlideDirection::Left, SlideDirection::Right);
    }

    #[test]
    fn test_transition_none() {
        let transition = Transition::None;
        assert!(transition.is_none());
        assert_eq!(transition.duration(), Duration::ZERO);
    }

    #[test]
    fn test_transition_fade() {
        let transition = Transition::fade(200);
        assert!(!transition.is_none());
        assert_eq!(transition.duration(), Duration::from_millis(200));
    }

    #[test]
    fn test_transition_slide() {
        let transition = Transition::slide_left(300);
        assert!(!transition.is_none());
        assert_eq!(transition.duration(), Duration::from_millis(300));

        if let Transition::Slide { direction, .. } = transition {
            assert_eq!(direction, SlideDirection::Left);
        } else {
            panic!("Expected Slide transition");
        }
    }

    #[test]
    fn test_transition_config_default() {
        let config = TransitionConfig::default();
        assert!(config.active().is_none());
        assert!(!config.has_override());
    }

    #[test]
    fn test_transition_config_with_default() {
        let config = TransitionConfig::new(Transition::fade(200));
        assert!(!config.active().is_none());
        assert!(!config.has_override());
    }

    #[test]
    fn test_transition_config_override() {
        let mut config = TransitionConfig::new(Transition::fade(200));

        config.set_override(Transition::slide_left(300));
        assert!(config.has_override());
        assert_eq!(config.active().duration(), Duration::from_millis(300));

        config.clear_override();
        assert!(!config.has_override());
        assert_eq!(config.active().duration(), Duration::from_millis(200));
    }

    #[test]
    fn test_transition_helpers() {
        // Test all helper methods
        let _ = Transition::fade(200);
        let _ = Transition::slide_left(300);
        let _ = Transition::slide_right(300);
        let _ = Transition::slide_up(300);
        let _ = Transition::slide_down(300);
    }
}
