//! Toasts: short, calm messages at the bottom of the screen. Errors from the API go through [`Toasts::api_error`],
//! which stays quiet for the two codes the client already handles (401 → login, upgrade → reload).

use std::time::Duration;

use leptos::prelude::*;
use nolabs_core::proto::ErrorCode;

use crate::api::ClientError;

const SHOW_FOR: Duration = Duration::from_millis(4_500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastAction {
    /// A new version is waiting: activate it and reload.
    Update,
}

impl ToastAction {
    pub fn label(self) -> &'static str {
        match self {
            ToastAction::Update => "Reload",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub text: String,
    pub action: Option<ToastAction>,
}

#[derive(Clone, Copy)]
pub struct Toasts {
    pub list: RwSignal<Vec<Toast>>,
    next_id: StoredValue<u64>,
}

impl Toasts {
    pub fn new() -> Toasts {
        Toasts { list: RwSignal::new(Vec::new()), next_id: StoredValue::new(1) }
    }

    fn push(&self, kind: ToastKind, text: impl Into<String>, action: Option<ToastAction>) {
        let text = text.into();
        // The same line twice in a row (e.g. offline on several calls) shows once.
        if self.list.with_untracked(|l| l.iter().any(|t| t.text == text)) {
            return;
        }
        let id = self.next_id.get_value();
        self.next_id.set_value(id + 1);
        self.list.update(|l| {
            l.push(Toast { id, kind, text, action });
            if l.len() > 3 {
                l.remove(0);
            }
        });
        if action.is_none() {
            let list = self.list;
            set_timeout(move || list.update(|l| l.retain(|t| t.id != id)), SHOW_FOR);
        }
    }

    pub fn info(&self, text: impl Into<String>) {
        self.push(ToastKind::Info, text, None);
    }

    pub fn success(&self, text: impl Into<String>) {
        self.push(ToastKind::Success, text, None);
    }

    pub fn error(&self, text: impl Into<String>) {
        self.push(ToastKind::Error, text, None);
    }

    pub fn api_error(&self, e: &ClientError) {
        if matches!(e.code(), Some(ErrorCode::Unauthorized | ErrorCode::UpgradeRequired)) {
            return;
        }
        self.error(e.friendly());
    }

    pub fn update_ready(&self) {
        self.push(ToastKind::Info, "A new version is ready.", Some(ToastAction::Update));
    }

    pub fn dismiss(&self, id: u64) {
        self.list.update(|l| l.retain(|t| t.id != id));
    }
}
