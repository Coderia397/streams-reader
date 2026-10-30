//! Recorded takes, one per script line. Samples stay in a local store (not a signal: they are large and never shown);
//! the UI reads the per-line summary.

use leptos::prelude::*;

use super::script::SCRIPT;
use crate::audio::analysis::{Issue, analyse, issues};
use crate::audio::wav::{encode_pcm16, join_lines, resample_linear};
use crate::interop::recorder::Take;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeInfo {
    pub duration_ms: u32,
    pub issues: Vec<Issue>,
}

impl TakeInfo {
    pub fn usable(&self) -> bool {
        !self.issues.iter().any(|i| i.blocking())
    }
}

#[derive(Clone, Copy)]
pub struct Takes {
    pub info: RwSignal<Vec<Option<TakeInfo>>>,
    samples: StoredValue<Vec<Option<Take>>, LocalStorage>,
}

impl Takes {
    pub fn new() -> Takes {
        Takes {
            info: RwSignal::new(vec![None; SCRIPT.len()]),
            samples: StoredValue::new_local((0..SCRIPT.len()).map(|_| None).collect()),
        }
    }

    pub fn set(&self, line: usize, take: Take) -> TakeInfo {
        let report = analyse(&take.samples, take.sample_rate);
        let info = TakeInfo { duration_ms: report.duration_ms, issues: issues(&report) };
        self.samples.update_value(|s| {
            if let Some(slot) = s.get_mut(line) {
                *slot = Some(take);
            }
        });
        let stored = info.clone();
        self.info.update(|v| {
            if let Some(slot) = v.get_mut(line) {
                *slot = Some(stored);
            }
        });
        info
    }

    pub fn all_usable(&self) -> bool {
        self.info.with(|v| v.iter().all(|t| t.as_ref().is_some_and(TakeInfo::usable)))
    }

    pub fn count(&self) -> usize {
        self.info.with(|v| v.iter().filter(|t| t.is_some()).count())
    }

    /// One WAV of every line in order, at the first take's sample rate.
    pub fn wav(&self) -> Option<Vec<u8>> {
        self.samples.with_value(|takes| {
            let takes: Vec<&Take> = takes.iter().map(Option::as_ref).collect::<Option<Vec<_>>>()?;
            let rate = takes.first()?.sample_rate;
            let lines: Vec<Vec<f32>> = takes.iter().map(|t| resample_linear(&t.samples, t.sample_rate, rate)).collect();
            Some(encode_pcm16(&join_lines(&lines, rate), rate))
        })
    }
}
