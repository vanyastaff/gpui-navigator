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
//! | [`Transition::Custom`] | [`Transition::custom`] | Your own enter/exit styling |
//! | [`Transition::Eased`] | [`Transition::with_easing`] | Any of the above with an easing curve |
//!
//! Each transition carries a `duration_ms` controlling animation length.
//!
//! Slides are mirrored automatically when navigating back (a `slide_left`
//! push is undone by a right slide); see
//! [`GlobalRouter::set_reverse_transitions_on_back`](crate::GlobalRouter::set_reverse_transitions_on_back).
//!
//! ```ignore
//! use gpui::Styled;
//! use gpui_navigator::{ease_out_cubic, Transition};
//!
//! // Ease a built-in transition
//! let slide = Transition::slide_left(250).with_easing(ease_out_cubic);
//!
//! // Or style the entering and leaving page yourself (progress goes 0 → 1)
//! let fade_up = Transition::custom(
//!     300,
//!     |page, p| page.opacity(p).top(gpui::px(24.0 * (1.0 - p))),
//!     |page, p| page.opacity(1.0 - p),
//! );
//! ```
//!
//! Transitions are plain data (functions, not closures with captures), so
//! they can be built in `const` items:
//!
//! ```ignore
//! const PAGE: Transition = Transition::slide_left(250).with_easing(gpui_navigator::ease_out_cubic);
//! ```
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

use gpui::{div, relative, Div, IntoElement, ParentElement, Styled};
use std::sync::Arc;
use std::time::Duration;

/// Styles a page layer for a given animation progress (`0.0..=1.0`).
///
/// A plain function (or non-capturing closure), so transitions stay cheap to
/// copy and usable in `const` contexts.
pub type TransitionStyle = fn(Div, f32) -> Div;

/// Maps linear animation progress (`0.0..=1.0`) to eased progress.
pub type Easing = fn(f32) -> f32;

/// A user-defined transition, see [`Transition::custom`].
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct CustomTransition {
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Styles the entering page; progress goes from 0 (start) to 1 (settled)
    pub enter: TransitionStyle,
    /// Styles the leaving page; progress goes from 0 (start) to 1 (gone)
    pub exit: TransitionStyle,
    /// Optional easing curve
    pub easing: Option<Easing>,
}

/// The built-in animation an [`Transition::Eased`] eases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EasedKind {
    /// Cross-fade
    Fade,
    /// Slide in the given direction
    Slide(SlideDirection),
}

type LayerStyle = Arc<dyn Fn(Div, f32) -> Div>;

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

impl SlideDirection {
    /// The direction pointing the other way.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
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

    /// User-defined enter/exit styling, see [`Transition::custom`]
    Custom(CustomTransition),

    /// A fade or slide played with an easing curve, see [`Transition::with_easing`]
    Eased {
        /// The animation being eased
        kind: EasedKind,
        /// Duration in milliseconds
        duration_ms: u64,
        /// The easing curve
        easing: Easing,
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

    /// Create a transition that styles the entering and leaving page yourself.
    ///
    /// Both functions receive the page's layer and the animation progress
    /// (`0.0` at the start, `1.0` when done). Non-capturing closures work.
    #[must_use]
    pub const fn custom(duration_ms: u64, enter: TransitionStyle, exit: TransitionStyle) -> Self {
        Self::Custom(CustomTransition {
            duration_ms,
            enter,
            exit,
            easing: None,
        })
    }

    /// Play this transition with an easing curve such as [`ease_out_cubic`].
    #[must_use]
    pub const fn with_easing(self, easing: Easing) -> Self {
        match self {
            Self::None => Self::None,
            Self::Fade { duration_ms } => Self::Eased {
                kind: EasedKind::Fade,
                duration_ms,
                easing,
            },
            Self::Slide {
                direction,
                duration_ms,
            } => Self::Eased {
                kind: EasedKind::Slide(direction),
                duration_ms,
                easing,
            },
            Self::Custom(mut custom) => {
                custom.easing = Some(easing);
                Self::Custom(custom)
            }
            Self::Eased {
                kind, duration_ms, ..
            } => Self::Eased {
                kind,
                duration_ms,
                easing,
            },
        }
    }

    /// The mirrored transition, used when navigating back: slides change
    /// direction, everything else stays the same.
    #[must_use]
    pub const fn reversed(&self) -> Self {
        match *self {
            Self::Slide {
                direction,
                duration_ms,
            } => Self::Slide {
                direction: direction.opposite(),
                duration_ms,
            },
            Self::Eased {
                kind: EasedKind::Slide(direction),
                duration_ms,
                easing,
            } => Self::Eased {
                kind: EasedKind::Slide(direction.opposite()),
                duration_ms,
                easing,
            },
            Self::None => Self::None,
            Self::Fade { duration_ms } => Self::Fade { duration_ms },
            Self::Custom(custom) => Self::Custom(custom),
            Self::Eased {
                kind,
                duration_ms,
                easing,
            } => Self::Eased {
                kind,
                duration_ms,
                easing,
            },
        }
    }

    /// Get the duration of this transition
    #[must_use]
    pub const fn duration(&self) -> Duration {
        match self {
            Self::None => Duration::ZERO,
            Self::Fade { duration_ms, .. }
            | Self::Slide { duration_ms, .. }
            | Self::Eased { duration_ms, .. }
            | Self::Custom(CustomTransition { duration_ms, .. }) => {
                Duration::from_millis(*duration_ms)
            }
        }
    }

    /// The easing curve, if any.
    #[must_use]
    pub const fn easing(&self) -> Option<Easing> {
        match self {
            Self::Eased { easing, .. } => Some(*easing),
            Self::Custom(custom) => custom.easing,
            _ => None,
        }
    }

    /// The enter and exit layer styles used to animate this transition, or
    /// `None` for [`Transition::None`].
    pub(crate) fn styles(&self) -> Option<(LayerStyle, LayerStyle)> {
        let slide = |direction: SlideDirection| -> (LayerStyle, LayerStyle) {
            // Enter starts one full width/height away, exit ends one away on
            // the other side.
            let (sign, vertical) = match direction {
                SlideDirection::Left => (1.0_f32, false),
                SlideDirection::Right => (-1.0, false),
                SlideDirection::Up => (1.0, true),
                SlideDirection::Down => (-1.0, true),
            };
            let offset = move |layer: Div, amount: f32| {
                if vertical {
                    layer.top(relative(amount))
                } else {
                    layer.left(relative(amount))
                }
            };
            (
                Arc::new(move |layer, p| offset(layer, sign * (1.0 - p))),
                Arc::new(move |layer, p| offset(layer, -sign * p)),
            )
        };
        let fade = || -> (LayerStyle, LayerStyle) {
            (
                Arc::new(|layer: Div, p: f32| layer.opacity(p)),
                Arc::new(|layer: Div, p: f32| layer.opacity(1.0 - p)),
            )
        };
        match self {
            Self::None => None,
            Self::Fade { .. }
            | Self::Eased {
                kind: EasedKind::Fade,
                ..
            } => Some(fade()),
            Self::Slide { direction, .. }
            | Self::Eased {
                kind: EasedKind::Slide(direction),
                ..
            } => Some(slide(*direction)),
            Self::Custom(custom) => {
                let (enter, exit) = (custom.enter, custom.exit);
                Some((Arc::new(enter), Arc::new(exit)))
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
    pub const fn set_override(&mut self, transition: Transition) {
        self.override_next = Some(transition);
    }

    /// Clear the override transition
    pub const fn clear_override(&mut self) {
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
/// styled like the entering page of the transition at that point: the same
/// styling `RouterOutlet` animates (fade sets the opacity, slides offset by a
/// fraction of the container's size, custom transitions use their enter
/// style; easing is applied for [`Transition::Eased`]).
pub fn apply_transition(element: impl IntoElement, transition: &Transition, progress: f32) -> Div {
    let layer = div().relative().size_full().child(element);
    match transition.styles() {
        None => layer,
        Some((enter, _)) => {
            let progress = progress.clamp(0.0, 1.0);
            let progress = transition.easing().map_or(progress, |ease| ease(progress));
            enter(layer, progress)
        }
    }
}

/// Linear easing (no easing).
#[must_use]
pub const fn linear(t: f32) -> f32 {
    t
}

/// Cubic ease-out easing: fast start, gentle stop.
#[must_use]
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease-in easing: gentle start, fast stop.
#[must_use]
pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
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
