use crate::bridge::events_js::serialize_event;
use napi_derive::napi;
use parking_lot::Mutex;
use screenshot_core::core::engine::Engine;
use std::sync::Arc;

#[napi]
pub struct ScreenshotSession {
    pub(crate) engine: Arc<Mutex<Engine>>,
}

#[napi]
impl ScreenshotSession {
    #[napi]
    pub fn cancel(&self) {
        self.engine.lock().cancel();
    }

    #[napi]
    pub fn poll_event(&self) -> Option<String> {
        let evt = self.engine.lock().event_bus.try_recv()?;
        Some(serialize_event(&evt))
    }
}
