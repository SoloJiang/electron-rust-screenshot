use crate::core::capture::{CaptureError, PlatformCapture, ScreenFrame};
use crate::core::types::Rect;
use core_graphics::display::{
    CGDisplayBounds, CGDisplayCreateImage, CGDisplayPixelsWide, CGGetActiveDisplayList,
};
use core_graphics::image::CGImage;
use foreign_types::ForeignType;
use image::RgbaImage;

pub struct MacOsCgCapture;

impl MacOsCgCapture {
    pub fn new() -> Self {
        Self
    }

    fn cg_image_to_rgba(img: &CGImage) -> RgbaImage {
        let width = img.width() as u32;
        let height = img.height() as u32;
        let mut rgba = RgbaImage::new(width, height);
        let data = img.data();
        let bytes = data.bytes();
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                let b = bytes[idx];
                let g = bytes[idx + 1];
                let r = bytes[idx + 2];
                let a = bytes[idx + 3];
                rgba.put_pixel(x, y, image::Rgba([r, g, b, a]));
            }
        }
        rgba
    }
}

impl PlatformCapture for MacOsCgCapture {
    fn capture_all_screens(&self) -> Result<Vec<ScreenFrame>, CaptureError> {
        let mut display_count: u32 = 0;
        let result = unsafe { CGGetActiveDisplayList(0, std::ptr::null_mut(), &mut display_count) };
        if result != 0 || display_count == 0 {
            return Err(CaptureError::NoDisplay);
        }
        let mut displays = vec![0u32; display_count as usize];
        let result = unsafe {
            CGGetActiveDisplayList(display_count, displays.as_mut_ptr(), &mut display_count)
        };
        if result != 0 {
            return Err(CaptureError::PlatformError("CGGetActiveDisplayList failed".into()));
        }

        let mut frames = Vec::with_capacity(display_count as usize);
        for id in displays {
            let bounds = unsafe { CGDisplayBounds(id) };
            let cg_img = unsafe { CGDisplayCreateImage(id) };
            if cg_img.is_null() {
                continue;
            }
            let cg_image = unsafe { CGImage::from_ptr(cg_img) };
            let rgba = Self::cg_image_to_rgba(&cg_image);
            let pixel_width = unsafe { CGDisplayPixelsWide(id) as f64 };
            let width = bounds.size.width;
            let dpi_scale = if width > 0.0 {
                pixel_width / width
            } else {
                1.0
            };

            frames.push(ScreenFrame {
                screen_id: id.to_string(),
                logical_bounds: Rect::new(
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                ),
                dpi_scale,
                image: rgba,
            });
        }

        if frames.is_empty() {
            return Err(CaptureError::NoDisplay);
        }
        Ok(frames)
    }
}
