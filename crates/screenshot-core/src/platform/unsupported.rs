use crate::core::types::DetectedWindow;
use crate::platform::traits::{PlatformClipboard, PlatformOverlay, PlatformWindowEnumerator};
use image::RgbaImage;
use winit::window::Window;

pub struct UnsupportedBackend;

impl PlatformClipboard for UnsupportedBackend {
    fn copy_image(_img: &RgbaImage) -> Result<(), String> {
        Err("Clipboard is not supported on this platform".into())
    }
}

impl PlatformOverlay for UnsupportedBackend {
    fn setup_window(_window: &Window) {
        // No platform-specific overlay setup is available.
    }

    fn set_mouse_passthrough(_window: &Window, _enable: bool) {
        // Mouse passthrough is unavailable on unsupported platforms.
    }
}

impl PlatformWindowEnumerator for UnsupportedBackend {
    fn enumerate_windows() -> Vec<DetectedWindow> {
        Vec::new()
    }
}
