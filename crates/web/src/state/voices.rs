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

/// Voices listed by id, in the order of `ids`, skipping unknown or unusable ones.
pub fn pick(voices: &[VoiceInfo], ids: &[String]) -> Vec<VoiceInfo> {
    ids.iter()
        .filter_map(|id| voices.iter().find(|v| &v.id == id && v.state == nolabs_core::proto::api::VoiceState::Ready))
        .cloned()
        .collect()
}

/// The caller's expressive budget today (D21): their own share, never more than what the whole deployment has
/// left, and the GPU runs left (shared by everyone).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuBudget {
    pub left_s: u32,
    pub share_s: u32,
    pub runs_left: u32,
}

impl GpuBudget {
    pub fn of(s: &SystemStatus) -> GpuBudget {
        let mine = s.my_gpu.share_s.saturating_sub(s.my_gpu.used_s);
        let everyone = s.gpu.cap_s.saturating_sub(s.gpu.used_s);
        GpuBudget {
            left_s: mine.min(everyone),
            share_s: s.my_gpu.share_s,
            runs_left: s.gpu.runs_cap.saturating_sub(s.gpu.runs_used),
        }
    }

    /// 0–100, for the bar.
    pub fn percent_left(self) -> f64 {
        f64::from(self.left_s) / f64::from(self.share_s.max(1)) * 100.0
    }

    pub fn is_spent(self) -> bool {
        self.left_s == 0 || self.runs_left == 0
    }
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
        self.find(id).map_or_else(|| t!("voice-yours-fallback"), |v| v.name)
    }
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::api::{Tier, VoiceState};

    use super::*;

    #[test]
    fn budget_is_the_users_share_capped_by_the_deployment() {
        let status: SystemStatus = serde_json::from_str(
            r#"{"version":"v","space":{"stage":"RUNNING","healthy":true,"checked_at_ms":0},
                "gpu":{"used_s":250,"cap_s":280,"runs_used":1,"runs_cap":3},
                "my_gpu":{"used_s":18,"share_s":60},"queued":0}"#,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let b = GpuBudget::of(&status);
        assert_eq!(b, GpuBudget { left_s: 30, share_s: 60, runs_left: 2 });
        assert_eq!(b.percent_left(), 50.0);
        assert!(!b.is_spent());
    }

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
    fn pick_keeps_order_and_skips_unknown() {
        let voices = builtin_catalog();
        let ids = vec![voices[3].id.clone(), "kokoro:nope".to_owned(), voices[0].id.clone()];
        let got: Vec<String> = pick(&voices, &ids).into_iter().map(|v| v.id).collect();
        assert_eq!(got, vec![voices[3].id.clone(), voices[0].id.clone()]);
    }

    #[test]
    fn preview_cost_estimate() {
        assert_eq!(preview_gpu_seconds(100), 14);
        assert_eq!(preview_gpu_seconds(0), 8);
    }
}
