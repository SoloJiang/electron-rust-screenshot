use super::types::Rect;
use image::RgbaImage;

pub trait PlatformCapture: Send + Sync {
    fn capture_all_screens(&self) -> Result<Vec<ScreenFrame>, CaptureError>;
}

#[derive(Debug, Clone)]
pub struct ScreenFrame {
    pub screen_id: String,
    pub logical_bounds: Rect,
    pub dpi_scale: f64,
    pub image: RgbaImage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaptureError {
    NoDisplay,
    PermissionDenied,
    PlatformError(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::NoDisplay => write!(f, "No display available"),
            CaptureError::PermissionDenied => write!(f, "Screen capture permission denied"),
            CaptureError::PlatformError(msg) => write!(f, "Platform error: {}", msg),
        }
    }
}

pub struct MockCapture {
    bounds: Rect,
}

impl MockCapture {
    pub fn new(bounds: Rect) -> Self {
        Self { bounds }
    }
}

impl PlatformCapture for MockCapture {
    fn capture_all_screens(&self) -> Result<Vec<ScreenFrame>, CaptureError> {
        let img = RgbaImage::new(self.bounds.w as u32, self.bounds.h as u32);
        Ok(vec![ScreenFrame {
            screen_id: "mock-1".to_string(),
            logical_bounds: self.bounds,
            dpi_scale: 1.0,
            image: img,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_capture_returns_one_screen() {
        let cap = MockCapture::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let frames = cap.capture_all_screens().unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].logical_bounds.w, 1920.0);
    }
}
