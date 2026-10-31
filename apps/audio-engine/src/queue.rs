//! The decode thread: owns the playlist of [`SegmentFile`]s, the decoder, the stretcher, the sleep fade and the
//! ring producer; reacts to [`Command`]s between chunks.
//!
//! Loop (TODO phase 2):
//! 1. Drain commands (non-blocking while playing, blocking while paused/idle).
//! 2. If the ring has room for a 20 ms chunk: take PCM from the current segment (decoding the next file when the
//!    current one is exhausted, so there is never a gap), apply gain (volume × sleep fade), stretch, push frames and
//!    a [`crate::clock::Tag`].
//! 3. When the next segment is missing: emit `Starved(n)` once and `State(Buffering)` when the ring runs dry.
//! 4. Every ~250 ms of output: emit `Position`.
//! 5. Seek: clear ring (generation counter checked by the callback), reset stretcher and clock, decode from the
//!    target.

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::thread;

use crate::{AudioError, Command, EngineState, Event, SegmentFile};

/// Ordered segments known to be on disk.
#[derive(Default)]
pub struct Playlist {
    files: BTreeMap<u32, SegmentFile>,
    pub segment_count: u32,
}

impl Playlist {
    pub fn insert(&mut self, file: SegmentFile) {
        self.files.insert(file.segment, file);
    }
    pub fn get(&self, segment: u32) -> Option<&SegmentFile> {
        self.files.get(&segment)
    }
    /// Milliseconds of audio available without a gap from `segment` (for `buffered_ahead_ms`).
    pub fn contiguous_ms_from(&self, segment: u32) -> u32 {
        let mut total = 0u32;
        let mut s = segment;
        while let Some(f) = self.files.get(&s) {
            total = total.saturating_add(f.duration_ms);
            s += 1;
        }
        total
    }
    pub fn clear(&mut self) {
        self.files.clear();
    }
}

/// Start the decode thread. Skeleton: it tracks the playlist and answers state changes; no audio yet.
pub fn spawn(commands: mpsc::Receiver<Command>, events: mpsc::Sender<Event>) -> Result<(), AudioError> {
    thread::Builder::new()
        .name("streams-audio-decode".into())
        .spawn(move || {
            let mut playlist = Playlist::default();
            while let Ok(cmd) = commands.recv() {
                let ev = match cmd {
                    Command::Open { segment_count, .. } => {
                        playlist.clear();
                        playlist.segment_count = segment_count;
                        Some(Event::State(EngineState::Buffering))
                    }
                    Command::Append(f) => {
                        playlist.insert(f);
                        None
                    }
                    Command::Play => Some(Event::State(EngineState::Playing)),
                    Command::Pause => Some(Event::State(EngineState::Paused)),
                    Command::Close => break,
                    _ => None,
                };
                if let Some(ev) = ev
                    && events.send(ev).is_err()
                {
                    break;
                }
            }
        })
        .map(|_| ())
        .map_err(|e| AudioError::Output(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn f(segment: u32, ms: u32) -> SegmentFile {
        SegmentFile { segment, path: PathBuf::from(format!("{segment}.webm")), duration_ms: ms, ends_section: false }
    }

    #[test]
    fn contiguous_stops_at_the_first_gap() {
        let mut p = Playlist::default();
        p.insert(f(0, 1000));
        p.insert(f(1, 2000));
        p.insert(f(3, 4000));
        assert_eq!(p.contiguous_ms_from(0), 3000);
        assert_eq!(p.contiguous_ms_from(2), 0);
        assert_eq!(p.contiguous_ms_from(3), 4000);
    }
}
