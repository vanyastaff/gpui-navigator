//! Navigation history management.
//!
//! This module provides a standalone [`History`] stack that is independent of
//! the route tree. It stores [`HistoryEntry`] values (path + optional
//! [`HistoryState`]) and supports:
//!
//! - **Push / replace** — add or overwrite the current entry.
//! - **Back / forward** — move a cursor through the stack.
//! - **Truncation** — forward entries are discarded on push (browser semantics).
//! - **Size limit** — configurable maximum via [`History::with_max_size`].
//! - **State persistence** — attach arbitrary key-value data (scroll position,
//!   form state, etc.) to each entry.
//! - **Serialization** — [`entries`](History::entries) /
//!   [`restore`](History::restore) for save/load workflows.
//!
//! # Examples
//!
//! ```
//! use gpui_navigator::history::History;
//!
//! let mut history = History::new("/".to_string());
//! history.push("/users".to_string());
//! history.push("/users/42".to_string());
//!
//! assert_eq!(history.current_path(), "/users/42");
//!
//! history.back();
//! assert_eq!(history.current_path(), "/users");
//! assert!(history.can_go_forward());
//! ```

use crate::NavigationDirection;

/// Navigation history entry
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    /// Path for this history entry
    pub path: String,
    /// Optional state data associated with this entry
    pub state: Option<HistoryState>,
}

impl HistoryEntry {
    /// Create a new history entry
    pub fn new(path: String) -> Self {
        Self { path, state: None }
    }

    /// Create with state
    pub fn with_state(path: String, state: HistoryState) -> Self {
        Self {
            path,
            state: Some(state),
        }
    }
}

/// State data for history entries
///
/// Can store arbitrary data for history restoration
/// (e.g., scroll position, form data, etc.)
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryState {
    /// Key-value pairs for state data
    pub data: std::collections::HashMap<String, String>,
}

impl HistoryState {
    /// Create new empty state
    pub fn new() -> Self {
        Self {
            data: std::collections::HashMap::new(),
        }
    }

    /// Set a value
    pub fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    /// Get a value
    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }
}

impl Default for HistoryState {
    fn default() -> Self {
        Self::new()
    }
}

/// Navigation history stack with configurable size limit.
///
/// Follows browser-like semantics: pushing a new entry truncates any forward
/// history, and an optional `max_size` evicts the oldest entries when exceeded.
#[derive(Debug, Clone)]
pub struct History {
    /// History stack.
    entries: Vec<HistoryEntry>,
    /// Current position (cursor) in the stack.
    current: usize,
    /// Maximum number of entries. `0` means unlimited.
    max_size: usize,
}

impl History {
    /// Create a new history with the given initial path and a default limit of 1000 entries.
    pub fn new(initial_path: String) -> Self {
        Self {
            entries: vec![HistoryEntry::new(initial_path)],
            current: 0,
            max_size: 1000, // Default limit
        }
    }

    /// Create a new history with a custom maximum size (`0` = unlimited).
    pub fn with_max_size(initial_path: String, max_size: usize) -> Self {
        Self {
            entries: vec![HistoryEntry::new(initial_path)],
            current: 0,
            max_size,
        }
    }

    /// Return the path of the current (cursor) entry.
    pub fn current_path(&self) -> &str {
        &self.entries[self.current].path
    }

    /// Return a reference to the current [`HistoryEntry`] (path + optional state).
    pub fn current_entry(&self) -> &HistoryEntry {
        &self.entries[self.current]
    }

    /// Push a new path onto history
    ///
    /// This truncates any forward history and adds the new entry
    pub fn push(&mut self, path: String) -> NavigationEvent {
        let from = Some(self.current_path().to_string());

        // Remove forward history when pushing
        self.entries.truncate(self.current + 1);

        // Add new entry
        self.entries.push(HistoryEntry::new(path.clone()));
        self.current += 1;

        // Enforce max size limit
        self.enforce_size_limit();

        NavigationEvent {
            from,
            to: path,
            direction: NavigationDirection::Forward,
        }
    }

    /// Push a new path with associated [`HistoryState`] data.
    pub fn push_with_state(&mut self, path: String, state: HistoryState) -> NavigationEvent {
        let from = Some(self.current_path().to_string());

        // Remove forward history
        self.entries.truncate(self.current + 1);

        // Add new entry with state
        self.entries
            .push(HistoryEntry::with_state(path.clone(), state));
        self.current += 1;

        self.enforce_size_limit();

        NavigationEvent {
            from,
            to: path,
            direction: NavigationDirection::Forward,
        }
    }

    /// Replace the current entry without modifying the stack length.
    pub fn replace(&mut self, path: String) -> NavigationEvent {
        let from = Some(self.current_path().to_string());

        self.entries[self.current] = HistoryEntry::new(path.clone());

        NavigationEvent {
            from,
            to: path,
            direction: NavigationDirection::Replace,
        }
    }

    /// Replace the current entry with a new path and [`HistoryState`].
    pub fn replace_with_state(&mut self, path: String, state: HistoryState) -> NavigationEvent {
        let from = Some(self.current_path().to_string());

        self.entries[self.current] = HistoryEntry::with_state(path.clone(), state);

        NavigationEvent {
            from,
            to: path,
            direction: NavigationDirection::Replace,
        }
    }

    /// Move the cursor one step back. Returns `None` if already at the oldest entry.
    pub fn back(&mut self) -> Option<NavigationEvent> {
        if self.can_go_back() {
            let from = Some(self.current_path().to_string());
            self.current -= 1;
            let to = self.current_path().to_string();

            Some(NavigationEvent {
                from,
                to,
                direction: NavigationDirection::Back,
            })
        } else {
            None
        }
    }

    /// Move the cursor one step forward. Returns `None` if already at the newest entry.
    pub fn forward(&mut self) -> Option<NavigationEvent> {
        if self.can_go_forward() {
            let from = Some(self.current_path().to_string());
            self.current += 1;
            let to = self.current_path().to_string();

            Some(NavigationEvent {
                from,
                to,
                direction: NavigationDirection::Forward,
            })
        } else {
            None
        }
    }

    /// Return `true` if [`back`](Self::back) would succeed.
    pub fn can_go_back(&self) -> bool {
        self.current > 0
    }

    /// Return `true` if [`forward`](Self::forward) would succeed.
    pub fn can_go_forward(&self) -> bool {
        self.current < self.entries.len() - 1
    }

    /// Clear all history and reset to a single entry at `initial_path`.
    pub fn clear(&mut self, initial_path: String) {
        self.entries.clear();
        self.entries.push(HistoryEntry::new(initial_path));
        self.current = 0;
    }

    /// Return the total number of entries in the stack.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if history is empty (should never be true in practice)
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return a slice of all entries (useful for serialization / persistence).
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Return the current cursor position in the stack.
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Restore history from a previously saved set of entries and cursor position.
    ///
    /// No-op if `entries` is empty or `current >= entries.len()`.
    pub fn restore(&mut self, entries: Vec<HistoryEntry>, current: usize) {
        if !entries.is_empty() && current < entries.len() {
            self.entries = entries;
            self.current = current;
        }
    }

    /// Enforce maximum size limit
    fn enforce_size_limit(&mut self) {
        if self.max_size > 0 && self.entries.len() > self.max_size {
            // Remove oldest entries, keeping the current path reachable
            let excess = self.entries.len() - self.max_size;
            self.entries.drain(0..excess);
            self.current = self.current.saturating_sub(excess);
        }
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new("/".to_string())
    }
}

/// Navigation event from history operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationEvent {
    /// Previous path
    pub from: Option<String>,
    /// New path
    pub to: String,
    /// Navigation direction
    pub direction: NavigationDirection,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_creation() {
        let history = History::new("/".to_string());
        assert_eq!(history.current_path(), "/");
        assert_eq!(history.len(), 1);
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_history_push() {
        let mut history = History::new("/".to_string());

        history.push("/users".to_string());
        assert_eq!(history.current_path(), "/users");
        assert_eq!(history.len(), 2);
        assert!(history.can_go_back());
        assert!(!history.can_go_forward());

        history.push("/users/123".to_string());
        assert_eq!(history.current_path(), "/users/123");
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_history_back_forward() {
        let mut history = History::new("/".to_string());
        history.push("/page1".to_string());
        history.push("/page2".to_string());

        assert_eq!(history.current_path(), "/page2");

        history.back();
        assert_eq!(history.current_path(), "/page1");
        assert!(history.can_go_back());
        assert!(history.can_go_forward());

        history.forward();
        assert_eq!(history.current_path(), "/page2");
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_history_truncation_on_push() {
        let mut history = History::new("/".to_string());
        history.push("/page1".to_string());
        history.push("/page2".to_string());
        history.back();

        assert_eq!(history.current_path(), "/page1");
        assert_eq!(history.len(), 3);

        // Push a new page - should truncate forward history
        history.push("/page3".to_string());
        assert_eq!(history.current_path(), "/page3");
        assert_eq!(history.len(), 3); // /, /page1, /page3
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_history_replace() {
        let mut history = History::new("/".to_string());
        history.push("/page1".to_string());

        history.replace("/page2".to_string());
        assert_eq!(history.current_path(), "/page2");
        assert_eq!(history.len(), 2); // Still 2 entries

        history.back();
        assert_eq!(history.current_path(), "/");
    }

    #[test]
    fn test_history_clear() {
        let mut history = History::new("/".to_string());
        history.push("/page1".to_string());
        history.push("/page2".to_string());

        history.clear("/home".to_string());
        assert_eq!(history.current_path(), "/home");
        assert_eq!(history.len(), 1);
        assert!(!history.can_go_back());
    }

    #[test]
    fn test_history_with_state() {
        let mut history = History::new("/".to_string());

        let mut state = HistoryState::new();
        state.set("scrollY".to_string(), "100".to_string());

        history.push_with_state("/page1".to_string(), state);

        let entry = history.current_entry();
        assert_eq!(entry.path, "/page1");
        assert!(entry.state.is_some());

        let saved_state = entry.state.as_ref().unwrap();
        assert_eq!(saved_state.get("scrollY"), Some(&"100".to_string()));
    }

    #[test]
    fn test_history_max_size() {
        let mut history = History::with_max_size("/".to_string(), 3);

        history.push("/page1".to_string());
        history.push("/page2".to_string());
        history.push("/page3".to_string()); // Should trigger limit
        history.push("/page4".to_string()); // Should remove oldest

        assert_eq!(history.len(), 3);
        assert_eq!(history.current_path(), "/page4");

        // Oldest entry "/" should be removed
        history.back();
        history.back();
        assert_eq!(history.current_path(), "/page2");
    }

    #[test]
    fn test_history_restore() {
        let mut history = History::new("/".to_string());

        let entries = vec![
            HistoryEntry::new("/".to_string()),
            HistoryEntry::new("/page1".to_string()),
            HistoryEntry::new("/page2".to_string()),
        ];

        history.restore(entries, 1);

        assert_eq!(history.len(), 3);
        assert_eq!(history.current_path(), "/page1");
        assert!(history.can_go_back());
        assert!(history.can_go_forward());
    }

    #[test]
    fn test_navigation_event() {
        let mut history = History::new("/".to_string());

        let event = history.push("/users".to_string());
        assert_eq!(event.from, Some("/".to_string()));
        assert_eq!(event.to, "/users");
        assert_eq!(event.direction, NavigationDirection::Forward);

        let event = history.back().unwrap();
        assert_eq!(event.from, Some("/users".to_string()));
        assert_eq!(event.to, "/");
        assert_eq!(event.direction, NavigationDirection::Back);
    }

    #[test]
    fn test_empty_history_boundaries() {
        let mut history = History::new("/".to_string());

        assert!(history.back().is_none());
        assert!(history.forward().is_none());
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }
}
