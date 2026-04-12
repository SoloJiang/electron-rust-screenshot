pub mod capture_cg;
pub mod capture_sck;
pub mod clipboard;
pub mod overlay_ext;
pub mod window;

use crate::core::types::DetectedWindow;
use crate::platform::traits::{PlatformClipboard, PlatformOverlay, PlatformWindowEnumerator};
use image::RgbaImage;
use winit::window::Window;

pub struct MacosBackend;

impl PlatformClipboard for MacosBackend {
    fn copy_image(img: &RgbaImage) -> Result<(), String> {
        clipboard::copy_image_to_clipboard(img)
    }
}

impl PlatformOverlay for MacosBackend {
    fn setup_window(window: &Window) {
        overlay_ext::setup_window(window);
    }

    fn set_mouse_passthrough(window: &Window, enable: bool) {
        overlay_ext::set_mouse_passthrough(window, enable);
    }
}

impl PlatformWindowEnumerator for MacosBackend {
    fn enumerate_windows() -> Vec<DetectedWindow> {
        window::enumerate_windows()
    }
}
