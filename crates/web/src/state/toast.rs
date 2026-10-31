//! Toasts: short, calm messages at the bottom of the screen, one at a time, at most one action (Undo, Reload).
//! Errors from the API go through [`Toasts::api_error`], which stays quiet for the two codes the client already
//! handles (401 → sign in, upgrade → reload).

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use leptos::prelude::*;
use nolabs_core::proto::ErrorCode;

use crate::api::ClientError;

/// Undo callbacks by toast id.
type UndoMap = HashMap<u64, Rc<dyn Fn()>>;

const SHOW_FOR: Duration = Duration::from_millis(4_000);
pub const SHOW_WITH_ACTION_FOR: Duration = Duration::from_millis(6_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastAction {
    /// A new version is waiting: activate it and reload. Stays until used or dismissed.
    Update,
    /// Reverts the change the toast reports (runs the callback stored for this toast).
    Undo,
}

impl ToastAction {
    pub fn label(self) -> String {
        match self {
            ToastAction::Update => t!("toast-reload"),
            ToastAction::Undo => t!("toast-undo"),
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
    undo: StoredValue<UndoMap, LocalStorage>,
}

impl Toasts {
    pub fn new() -> Toasts {
        Toasts {
            list: RwSignal::new(Vec::new()),
            next_id: StoredValue::new(1),
            undo: StoredValue::new_local(HashMap::new()),
        }
    }

    fn push(&self, kind: ToastKind, text: String, action: Option<ToastAction>) -> u64 {
        // The same line twice in a row (e.g. offline on several calls) shows once.
        if let Some(id) = self.list.with_untracked(|l| l.iter().find(|t| t.text == text).map(|t| t.id)) {
            return id;
        }
        let id = self.next_id.get_value();
        self.next_id.set_value(id + 1);
        // One visible at a time: a new toast replaces the old one (an update notice is kept).
        self.list.update(|l| {
            l.retain(|t| t.action == Some(ToastAction::Update));
            l.push(Toast { id, kind, text, action });
        });
        let stay = match action {
            Some(ToastAction::Update) => None,
            Some(ToastAction::Undo) => Some(SHOW_WITH_ACTION_FOR),
            None => Some(SHOW_FOR),
        };
        if let Some(after) = stay {
            let me = *self;
            set_timeout(move || me.dismiss(id), after);
        }
        id
    }

    pub fn info(&self, text: impl Into<String>) {
        self.push(ToastKind::Info, text.into(), None);
    }

    pub fn success(&self, text: impl Into<String>) {
        self.push(ToastKind::Success, text.into(), None);
    }

    pub fn error(&self, text: impl Into<String>) {
        self.push(ToastKind::Error, text.into(), None);
    }

    /// A change that applied at once, with Undo.
    pub fn undoable(&self, text: impl Into<String>, undo: impl Fn() + 'static) {
        let id = self.push(ToastKind::Info, text.into(), Some(ToastAction::Undo));
        self.undo.update_value(|m| {
            m.insert(id, Rc::new(undo));
        });
    }

    pub fn api_error(&self, e: &ClientError) {
        if matches!(e.code(), Some(ErrorCode::Unauthorized | ErrorCode::UpgradeRequired)) {
            return;
        }
        self.error(e.friendly());
    }

    pub fn update_ready(&self) {
        self.push(ToastKind::Info, t!("toast-update-ready"), Some(ToastAction::Update));
    }

    /// Runs the Undo stored for a toast and closes it.
    pub fn run_undo(&self, id: u64) {
        let f = self.undo.try_update_value(|m| m.remove(&id)).flatten();
        self.dismiss(id);
        if let Some(f) = f {
            f();
        }
    }

    pub fn dismiss(&self, id: u64) {
        self.list.update(|l| l.retain(|t| t.id != id));
        self.undo.update_value(|m| {
            m.remove(&id);
        });
    }
}
