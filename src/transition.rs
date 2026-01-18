//! Route transition animations
//!
//! This module provides a transition system for route changes,
//! allowing separate enter and exit animations for incoming and outgoing content.

use gpui::{div, px, Div, IntoElement, ParentElement, Styled};
use std::time::Duration;

/// Direction for slide transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Built-in transition types
#[derive(Default)]
pub enum Transition {
    /// No transition animation
    #[default]
    None,

    /// Fade transition (simple opacity animation)
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

impl std::fmt::Debug for Transition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "Transition::None"),
            Self::Fade { duration_ms } => f
                .debug_struct("Transition::Fade")
                .field("duration_ms", duration_ms)
                .finish(),
            Self::Slide {
                direction,
                duration_ms,
            } => f
                .debug_struct("Transition::Slide")
                .field("direction", direction)
                .field("duration_ms", duration_ms)
                .finish(),
        }
    }
}

impl Clone for Transition {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Fade { duration_ms } => Self::Fade {
                duration_ms: *duration_ms,
            },
            Self::Slide {
                direction,
                duration_ms,
            } => Self::Slide {
                direction: *direction,
                duration_ms: *duration_ms,
            },
        }
    }
}

impl Transition {
    /// Create a fade transition
    pub fn fade(duration_ms: u64) -> Self {
        Self::Fade { duration_ms }
    }

    /// Create a slide-left transition
    pub fn slide_left(duration_ms: u64) -> Self {
        Self::Slide {
            direction: SlideDirection::Left,
            duration_ms,
        }
    }

    /// Create a slide-right transition
    pub fn slide_right(duration_ms: u64) -> Self {
        Self::Slide {
            direction: SlideDirection::Right,
            duration_ms,
        }
    }

    /// Create a slide-up transition
    pub fn slide_up(duration_ms: u64) -> Self {
        Self::Slide {
            direction: SlideDirection::Up,
            duration_ms,
        }
    }

    /// Create a slide-down transition
    pub fn slide_down(duration_ms: u64) -> Self {
        Self::Slide {
            direction: SlideDirection::Down,
            duration_ms,
        }
    }

    /// Get the duration of this transition
    pub fn duration(&self) -> Duration {
        match self {
            Self::None => Duration::ZERO,
            Self::Fade { duration_ms, .. } => Duration::from_millis(*duration_ms),
            Self::Slide { duration_ms, .. } => Duration::from_millis(*duration_ms),
        }
    }

    /// Check if this is a no-op transition
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Transition configuration for route navigation
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
    pub fn new(default: Transition) -> Self {
        Self {
            default,
            override_next: None,
        }
    }

    /// Get the active transition (override if set, otherwise default)
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
    pub fn has_override(&self) -> bool {
        self.override_next.is_some()
    }
}

// ============================================================================
// Transition Builder
// ============================================================================

/// Transition context passed to transition builder
pub struct TransitionContext {
    /// Animation progress from 0.0 to 1.0
    pub animation: f32,
    /// Secondary animation for exit transitions (1.0 to 0.0)
    pub secondary_animation: f32,
}

/// Applies transition effect to element based on Transition type
///
/// Takes an element, a transition type, and a progress value (0.0 to 1.0),
/// then returns a `Div` with the appropriate visual transformation applied.
pub fn apply_transition(element: impl IntoElement, transition: &Transition, progress: f32) -> Div {
    // Always use consistent method chain to avoid recursion limit
    // Calculate all values first, then apply them in one chain
    let (x, y, opacity) = match transition {
        Transition::None => (0.0, 0.0, 1.0),

        Transition::Fade { .. } => {
            // Simple fade in effect
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

/// Easing function - ease in out cubic
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Apply easing to progress
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
