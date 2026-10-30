//! Host build of the driver. The player needs a browser; on the host it only exists so the pure modules and the
//! public types build and test natively. [`Driver::new`] always fails, so the other methods are never reached.

use std::rc::Rc;

use crate::{MediaInfo, PlayerError, PlayerEvent, PlayerState, Provider, Sleep, Snapshot};

pub(crate) struct Driver;

impl Driver {
    pub(crate) fn new(_provider: Provider, _on_event: Rc<dyn Fn(PlayerEvent)>) -> Result<Driver, PlayerError> {
        Err(PlayerError::Unsupported("the player runs only in a browser (wasm32)".into()))
    }
    pub(crate) fn load(&self, _info: MediaInfo, _segment_count: u32, _start_segment: u32) {}
    pub(crate) fn play(&self) {}
    pub(crate) fn pause(&self) {}
    pub(crate) fn toggle(&self) {}
    pub(crate) fn seek_by(&self, _delta_ms: i32) {}
    pub(crate) fn jump_to(&self, _segment: u32) {}
    pub(crate) fn next(&self) {}
    pub(crate) fn previous(&self) {}
    pub(crate) fn set_rate(&self, _rate: f64) {}
    pub(crate) fn set_sleep(&self, _sleep: Sleep) {}
    pub(crate) fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: PlayerState::Idle,
            segment: 0,
            segment_pos_ms: 0,
            word: None,
            buffered_ahead_ms: 0,
            rate: 1.0,
            sleep_left_ms: None,
        }
    }
    pub(crate) fn unload(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_build_reports_unsupported() {
        let provider: Provider = Rc::new(|_| Box::pin(async { Err(PlayerError::Other("unused".into())) }));
        let result = crate::Player::new(provider, Rc::new(|_| {}));
        assert!(matches!(result, Err(PlayerError::Unsupported(_))));
    }
}
