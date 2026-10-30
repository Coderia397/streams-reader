//! The voice catalogue and the Worker's status (GPU budget). Falls back to the built-in catalogue offline, so the
//! voice sheet always has the Standard voices.

use leptos::prelude::*;
use nolabs_core::proto::api::{Engine, SystemStatus, VoiceCatalog, VoiceInfo, VoiceKind};
use nolabs_core::voices::{DEFAULT_VOICE, builtin_catalog};

use super::toast::Toasts;
use crate::api::{status, voices};

#[derive(Clone, Copy)]
pub struct VoicesState {
    pub catalog: RwSignal<Option<VoiceCatalog>>,
    pub status: RwSignal<Option<SystemStatus>>,
}

/// Sections of the voice list, in display order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grouped {
    pub mine: Vec<VoiceInfo>,
    pub standard_featured: Vec<VoiceInfo>,
    pub standard_more: Vec<VoiceInfo>,
    pub expressive: Vec<VoiceInfo>,
}

pub fn group(voices: &[VoiceInfo]) -> Grouped {
    let mut g = Grouped::default();
    for v in voices {
        let slot = match (v.kind, v.engine, v.featured) {
            (VoiceKind::Clone | VoiceKind::Designed, _, _) => &mut g.mine,
            (VoiceKind::Preset, Engine::Kokoro, true) => &mut g.standard_featured,
            (VoiceKind::Preset, Engine::Kokoro, false) => &mut g.standard_more,
            (VoiceKind::Preset, Engine::Ming, _) => &mut g.expressive,
        };
        slot.push(v.clone());
    }
    g
}

/// Seconds of GPU left today and runs left, from the status view.
pub fn gpu_left(s: &SystemStatus) -> (u32, u32) {
    (s.gpu.cap_s.saturating_sub(s.gpu.used_s), s.gpu.runs_cap.saturating_sub(s.gpu.runs_used))
}

/// Rough GPU seconds one preview costs (Ming: ~0.06 s per char plus warm-up; see notes/08).
pub fn preview_gpu_seconds(chars: usize) -> u32 {
    8 + (chars as f64 * 0.06).ceil() as u32
}

impl VoicesState {
    pub fn new() -> VoicesState {
        VoicesState { catalog: RwSignal::new(None), status: RwSignal::new(None) }
    }

    pub async fn refresh(self, toasts: Toasts) {
        match voices::catalog().await {
            Ok(c) => self.catalog.set(Some(c)),
            Err(e) => {
                if self.catalog.with_untracked(Option::is_none) {
                    self.catalog
                        .set(Some(VoiceCatalog { voices: builtin_catalog(), default_voice: DEFAULT_VOICE.into() }));
                }
                if !e.is_offline() {
                    toasts.api_error(&e);
                }
            }
        }
    }

    pub async fn refresh_status(self) {
        if let Ok(s) = status::status().await {
            self.status.set(Some(s));
        }
    }

    pub fn find(&self, id: &str) -> Option<VoiceInfo> {
        let from_catalog =
            self.catalog.with(|c| c.as_ref().and_then(|c| c.voices.iter().find(|v| v.id == id).cloned()));
        from_catalog.or_else(|| builtin_catalog().into_iter().find(|v| v.id == id))
    }

    /// Display name; user voices fall back to "Your voice" until the catalogue loads.
    pub fn name(&self, id: &str) -> String {
        self.find(id).map_or_else(|| "Your voice".to_owned(), |v| v.name)
    }
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::{Tier, VoiceState};

    use super::*;

    #[test]
    fn groups_the_builtin_catalogue() {
        let mut voices = builtin_catalog();
        voices.push(VoiceInfo {
            id: "user:0123456789abcdef".into(),
            engine: Engine::Ming,
            tier: Tier::Expressive,
            kind: VoiceKind::Clone,
            name: "Me".into(),
            gender: None,
            accent: None,
            description: String::new(),
            featured: false,
            state: VoiceState::Ready,
            preview_key: None,
        });
        let g = group(&voices);
        assert_eq!(g.standard_featured.len(), 9);
        assert_eq!(g.standard_more.len(), 19);
        assert_eq!(g.expressive.len(), 5);
        assert_eq!(g.mine.len(), 1);
    }

    #[test]
    fn preview_cost_estimate() {
        assert_eq!(preview_gpu_seconds(100), 14);
        assert_eq!(preview_gpu_seconds(0), 8);
    }
}
