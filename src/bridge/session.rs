use crate::bridge::events_js::serialize_event;
use crate::core::engine::Engine;
use napi_derive::napi;
use std::sync::{Arc, Mutex};

#[napi]
pub struct ScreenshotSession {
    pub(crate) engine: Arc<Mutex<Engine>>,
}

#[napi]
impl ScreenshotSession {
    #[napi]
    pub fn cancel(&self) {
        self.engine.lock().unwrap().cancel();
    }

    #[napi]
    pub fn poll_event(&self) -> Option<String> {
        let evt = self.engine.lock().unwrap().event_bus.try_recv()?;
        Some(serialize_event(&evt))
    }
}
