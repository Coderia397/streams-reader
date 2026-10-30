//! Navigation from anywhere (async tasks, sheets, the API's auth hook). The router's navigate function is installed
//! once from inside `<Router>`; this `Copy` handle forwards to it.

use std::rc::Rc;

use leptos::prelude::*;
use leptos_router::NavigateOptions;

type NavFn = Rc<dyn Fn(&str, NavigateOptions)>;

#[derive(Clone, Copy)]
pub struct Nav {
    f: StoredValue<Option<NavFn>, LocalStorage>,
}

impl Nav {
    pub fn new() -> Nav {
        Nav { f: StoredValue::new_local(None) }
    }

    pub fn install(&self, f: impl Fn(&str, NavigateOptions) + 'static) {
        self.f.set_value(Some(Rc::new(f)));
    }

    pub fn go(&self, path: &str) {
        if let Some(f) = self.f.get_value() {
            f(path, NavigateOptions::default());
        }
    }

    /// Navigates without adding a history entry (redirects).
    pub fn replace(&self, path: &str) {
        if let Some(f) = self.f.get_value() {
            f(path, NavigateOptions { replace: true, ..NavigateOptions::default() });
        }
    }
}
