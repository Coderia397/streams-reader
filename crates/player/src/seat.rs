//! Which player the page's Media Session belongs to.
//!
//! Action handlers, metadata, playback state and position state are global to the page, while the web app may hold
//! two players for a moment (it creates the next document's player before it drops the old one). Only the player that
//! holds the seat writes to the session; a player takes it when it is loaded or asked to play, and leaving frees it
//! only when the leaving player still holds it, so an old player's teardown never removes the new player's handlers.

/// Player ids and the current holder. One per page (the web driver keeps it in a `thread_local`).
#[derive(Debug, Default)]
pub(crate) struct Seat {
    last_id: u64,
    holder: Option<u64>,
}

impl Seat {
    /// A new id for a player that does not hold the seat yet.
    pub(crate) fn join(&mut self) -> u64 {
        self.last_id += 1;
        self.last_id
    }

    /// Gives the seat to `id`. True when it changed hands, so the caller must install its handlers and state.
    pub(crate) fn claim(&mut self, id: u64) -> bool {
        let changed = self.holder != Some(id);
        self.holder = Some(id);
        changed
    }

    pub(crate) fn holds(&self, id: u64) -> bool {
        self.holder == Some(id)
    }

    /// `id` is going away. True when it held the seat, so the caller must clear the session; false leaves the
    /// holder's handlers and metadata alone.
    pub(crate) fn leave(&mut self, id: u64) -> bool {
        let held = self.holds(id);
        if held {
            self.holder = None;
        }
        held
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_distinct_and_nobody_holds_at_first() {
        let mut seat = Seat::default();
        let a = seat.join();
        let b = seat.join();
        assert_ne!(a, b);
        assert!(!seat.holds(a) && !seat.holds(b));
    }

    #[test]
    fn claim_reports_a_change_of_hands_once() {
        let mut seat = Seat::default();
        let a = seat.join();
        assert!(seat.claim(a));
        assert!(!seat.claim(a));
        assert!(seat.holds(a));
    }

    /// The web app's order: new player B, old player A dropped, then B loaded.
    #[test]
    fn old_player_dropped_before_new_one_loads() {
        let mut seat = Seat::default();
        let a = seat.join();
        seat.claim(a);
        let b = seat.join();
        assert!(seat.holds(a), "creating B leaves A in the seat");
        assert!(seat.leave(a), "A held the seat, so its teardown clears the session");
        assert!(seat.claim(b), "B installs its handlers on load");
        assert!(seat.holds(b));
    }

    /// B loaded before A is dropped: A's teardown must not touch B's handlers.
    #[test]
    fn old_player_dropped_after_new_one_loads() {
        let mut seat = Seat::default();
        let a = seat.join();
        seat.claim(a);
        let b = seat.join();
        assert!(seat.claim(b));
        assert!(!seat.leave(a));
        assert!(seat.holds(b));
    }

    /// A player created and dropped without being loaded (an open that failed) leaves the live one in the seat.
    #[test]
    fn unloaded_player_leaving_keeps_the_holder() {
        let mut seat = Seat::default();
        let a = seat.join();
        seat.claim(a);
        let b = seat.join();
        assert!(!seat.leave(b));
        assert!(seat.holds(a));
    }

    #[test]
    fn a_player_takes_the_seat_back() {
        let mut seat = Seat::default();
        let a = seat.join();
        let b = seat.join();
        seat.claim(a);
        seat.claim(b);
        assert!(seat.claim(a));
        assert!(seat.holds(a) && !seat.holds(b));
    }
}
