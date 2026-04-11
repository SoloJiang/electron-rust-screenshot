use crate::core::capture::{CaptureError, PlatformCapture, ScreenFrame};

pub struct MacOsSckCapture;

impl MacOsSckCapture {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformCapture for MacOsSckCapture {
    fn capture_all_screens(&self) -> Result<Vec<ScreenFrame>, CaptureError> {
        // TODO: Implement ScreenCaptureKit via objc bindings
        // For now, fallback to CGDisplay to keep build green
        use crate::platform::macos::capture_cg::MacOsCgCapture;
        MacOsCgCapture::new().capture_all_screens()
    }
}
