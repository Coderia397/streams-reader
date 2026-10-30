//! Poll delays for queued renders: 1, 2, 4, 8 s, then every 15 s. Doubling keeps the first answers quick (Kokoro
//! renders a window in a few seconds) while a long Ming job does not hammer the Worker.

const FIRST_S: u32 = 1;
const CAP_S: u32 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    next_s: u32,
}

impl Default for Backoff {
    fn default() -> Self {
        Backoff { next_s: FIRST_S }
    }
}

impl Backoff {
    /// The delay to wait now, in milliseconds; advances the sequence.
    pub fn next_ms(&mut self) -> u32 {
        let now = self.next_s;
        self.next_s = (self.next_s * 2).min(CAP_S);
        now * 1000
    }

    /// Like [`Self::next_ms`] but never shorter than a server hint (`retry_after_s`), still capped at 15 s so the
    /// app notices when the hint was pessimistic.
    pub fn next_ms_with_hint(&mut self, retry_after_s: Option<u64>) -> u32 {
        let base = self.next_ms();
        match retry_after_s {
            Some(s) => base.max((s.min(u64::from(CAP_S)) as u32) * 1000),
            None => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_then_caps_at_fifteen_seconds() {
        let mut b = Backoff::default();
        let seq: Vec<u32> = (0..7).map(|_| b.next_ms()).collect();
        assert_eq!(seq, vec![1000, 2000, 4000, 8000, 15000, 15000, 15000]);
    }

    #[test]
    fn hint_raises_but_never_exceeds_cap() {
        let mut b = Backoff::default();
        assert_eq!(b.next_ms_with_hint(Some(5)), 5000);
        assert_eq!(b.next_ms_with_hint(Some(3600)), 15000);
        assert_eq!(b.next_ms_with_hint(None), 4000);
    }
}
