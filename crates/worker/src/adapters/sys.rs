//! [`Clock`] and [`Rng`] from the Workers runtime.

use nolabs_server::ports::{Clock, PortError, Rng};
use worker::js_sys;
use worker::wasm_bindgen::JsCast;

pub struct WorkerClock;

impl Clock for WorkerClock {
    fn now_ms(&self) -> i64 {
        i64::try_from(worker::Date::now().as_millis()).unwrap_or(i64::MAX)
    }
}

/// `crypto.getRandomValues` of the Workers global scope.
pub struct CryptoRng;

impl Rng for CryptoRng {
    fn fill(&self, buf: &mut [u8]) -> Result<(), PortError> {
        let crypto = js_sys::Reflect::get(&js_sys::global(), &"crypto".into())
            .ok()
            .filter(|c| c.is_object())
            // Not `dyn_into`: the instanceof check is not guaranteed for the runtime's global `crypto`.
            .map(|c| c.unchecked_into::<web_sys::Crypto>())
            .ok_or_else(|| PortError::new("crypto is not available"))?;
        crypto.get_random_values_with_u8_array(buf).map(|_| ()).map_err(|_| PortError::new("getRandomValues failed"))
    }
}
