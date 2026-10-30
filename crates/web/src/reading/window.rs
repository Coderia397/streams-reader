//! Which display chunks the reader renders. A long book has thousands of blocks; rendering only a few chunks around
//! the position keeps scrolling and highlighting fast. The window grows as the user scrolls into a sentinel and is
//! re-centred when playback moves elsewhere.

/// Chunks kept in the DOM at most.
pub const MAX_CHUNKS: usize = 4;

/// Chunks `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: usize,
    pub end: usize,
}

impl Window {
    /// One chunk before `center` and two after (reading runs forward).
    pub fn around(center: usize, total: usize) -> Window {
        if total == 0 {
            return Window { start: 0, end: 0 };
        }
        let center = center.min(total - 1);
        let start = center.saturating_sub(1);
        let end = (center + 3).min(total);
        Window { start, end }
    }

    pub fn contains(&self, chunk: usize) -> bool {
        (self.start..self.end).contains(&chunk)
    }

    /// Adds the chunk above; drops chunks at the bottom beyond [`MAX_CHUNKS`].
    pub fn grow_up(self) -> Window {
        let start = self.start.saturating_sub(1);
        Window { start, end: self.end.min(start + MAX_CHUNKS) }
    }

    /// Adds the chunk below; drops chunks at the top beyond [`MAX_CHUNKS`].
    pub fn grow_down(self, total: usize) -> Window {
        let end = (self.end + 1).min(total);
        Window { start: self.start.max(end.saturating_sub(MAX_CHUNKS)), end }
    }

    pub fn at_top(&self) -> bool {
        self.start == 0
    }

    pub fn at_bottom(&self, total: usize) -> bool {
        self.end >= total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn around_clamps_to_the_document() {
        assert_eq!(Window::around(0, 10), Window { start: 0, end: 3 });
        assert_eq!(Window::around(5, 10), Window { start: 4, end: 8 });
        assert_eq!(Window::around(9, 10), Window { start: 8, end: 10 });
        assert_eq!(Window::around(50, 3), Window { start: 1, end: 3 });
        assert_eq!(Window::around(0, 0), Window { start: 0, end: 0 });
    }

    #[test]
    fn growing_keeps_at_most_max_chunks() {
        let w = Window { start: 4, end: 8 };
        assert_eq!(w.grow_up(), Window { start: 3, end: 7 });
        assert_eq!(w.grow_down(20), Window { start: 5, end: 9 });
        assert_eq!(Window { start: 0, end: 2 }.grow_up(), Window { start: 0, end: 2 });
        assert_eq!(Window { start: 7, end: 10 }.grow_down(10), Window { start: 7, end: 10 });
        assert_eq!(Window { start: 1, end: 3 }.grow_down(10), Window { start: 1, end: 4 });
    }

    #[test]
    fn edges() {
        let w = Window { start: 0, end: 10 };
        assert!(w.at_top());
        assert!(w.at_bottom(10));
        assert!(!w.at_bottom(11));
        assert!(w.contains(9));
        assert!(!w.contains(10));
    }
}
