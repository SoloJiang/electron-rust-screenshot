use crate::core::capture::{CaptureError, PlatformCapture};
use crate::core::types::DetectedWindow;
use image::RgbaImage;
use winit::window::Window;

/// Clipboard operations that vary by platform.
pub trait PlatformClipboard {
    fn copy_image(img: &RgbaImage) -> Result<(), String>;
}

/// Overlay window behaviors that require platform-specific APIs.
pub trait PlatformOverlay {
    /// Perform one-time window setup after creation (level, activation policy, etc.).
    fn setup_window(window: &Window);
    /// Enable or disable mouse-event passthrough for the window.
    fn set_mouse_passthrough(window: &Window, enable: bool);
}

/// Window enumeration that varies by platform.
pub trait PlatformWindowEnumerator {
    fn enumerate_windows() -> Vec<DetectedWindow>;
}

/// Factory for creating the screen-capture backend for the current platform.
pub fn create_capture() -> Result<Box<dyn PlatformCapture>, CaptureError> {
    #[cfg(target_os = "macos")]
    {
        use crate::platform::macos::capture_sck::MacOsSckCapture;
        Ok(Box::new(MacOsSckCapture::new()))
    }
    #[cfg(target_os = "windows")]
    {
        // Stub until Windows capture is implemented.
        Err(CaptureError::PlatformError(
            "Windows capture not yet implemented".into(),
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(CaptureError::PlatformError("Unsupported platform".into()))
    }
}
