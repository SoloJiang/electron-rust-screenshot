use crate::core::types::DetectedWindow;
use crate::platform::traits::{PlatformClipboard, PlatformOverlay, PlatformWindowEnumerator};
use image::RgbaImage;
use winit::window::Window;

pub struct WindowsBackend;

impl PlatformClipboard for WindowsBackend {
    fn copy_image(_img: &RgbaImage) -> Result<(), String> {
        Err("Windows clipboard not yet implemented".into())
    }
}

impl PlatformOverlay for WindowsBackend {
    fn setup_window(_window: &Window) {
        // TODO: Windows overlay window setup (topmost, no taskbar button, etc.)
    }

    fn set_mouse_passthrough(_window: &Window, _enable: bool) {
        // TODO: Windows WS_EX_TRANSPARENT / SetWindowRgn or layered window
    }
}

impl PlatformWindowEnumerator for WindowsBackend {
    fn enumerate_windows() -> Vec<DetectedWindow> {
        // TODO: EnumWindows on Windows
        Vec::new()
    }
}
