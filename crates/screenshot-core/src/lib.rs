#![deny(clippy::all)]

pub mod core;
pub mod overlay;
pub mod platform;

// Re-export commonly used items for convenience
pub use core::capture::{CaptureError, PlatformCapture, ScreenFrame};
pub use core::engine::Engine;
pub use core::events::EngineEvent;
pub use core::types::{Color, LogicalPoint, Rect, ScreenInfo, DetectedWindow};
pub use core::window::WindowDetector;
pub use overlay::manager::OverlayManager;
