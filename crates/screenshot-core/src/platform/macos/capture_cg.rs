use crate::core::capture::{CaptureError, PlatformCapture, ScreenFrame};
use crate::core::types::Rect;
use core_graphics::display::{CGDisplayBounds, CGDisplayCreateImage, CGGetActiveDisplayList};
use core_graphics::image::CGImage;
use foreign_types::ForeignType;
use image::RgbaImage;

pub struct MacOsCgCapture;

impl Default for MacOsCgCapture {
    fn default() -> Self {
        Self::new()
    }
}

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
        let bytes_per_row = img.bytes_per_row();
        for y in 0..height {
            for x in 0..width {
                let idx = (y as usize * bytes_per_row) + (x as usize * 4);
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
            return Err(CaptureError::PlatformError(
                "CGGetActiveDisplayList failed".into(),
            ));
        }

        let mut frames = Vec::with_capacity(display_count as usize);
        for id in displays {
            let bounds = unsafe { CGDisplayBounds(id) };
            let cg_img = unsafe { CGDisplayCreateImage(id) };
            if cg_img.is_null() {
                eprintln!(
                    "[capture_cg] display {}: CGDisplayCreateImage returned null",
                    id
                );
                continue;
            }
            let cg_image = unsafe { CGImage::from_ptr(cg_img) };
            let rgba = Self::cg_image_to_rgba(&cg_image);
            let width = bounds.size.width;
            let height = bounds.size.height;
            // CGDisplayPixelsWide is unreliable on Retina/rotated displays;
            // derive dpi_scale directly from the captured image vs logical bounds.
            let dpi_scale = if width > 0.0 {
                rgba.width() as f64 / width
            } else {
                1.0
            };
            let rotation = unsafe { core_graphics::display::CGDisplayRotation(id) };
            eprintln!(
                "[capture_cg] display {}: bounds={:.0},{:.0} {:.0}x{:.0} img={}x{} rotation={:.0} dpi_scale={:.3}",
                id,
                bounds.origin.x,
                bounds.origin.y,
                width,
                height,
                rgba.width(),
                rgba.height(),
                rotation,
                dpi_scale
            );

            // Note: CGDisplayCreateImage already returns the image in the
            // user-visible orientation (matches logical bounds), so no explicit
            // rotation is needed even for portrait displays.

            // Sanity-check aspect ratio to catch unexpected rotation behavior.
            if width > 0.0 && height > 0.0 {
                let logical_aspect = width / height;
                let image_aspect = rgba.width() as f64 / rgba.height() as f64;
                let aspect_diff = (logical_aspect - image_aspect).abs();
                if aspect_diff > 0.05 * logical_aspect.max(image_aspect) {
                    eprintln!(
                        "[capture_cg] display {}: WARNING aspect mismatch logical={:.3} image={:.3} rotation={:.0}. CGDisplayCreateImage may not be pre-rotated.",
                        id, logical_aspect, image_aspect, rotation
                    );
                }
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn debug_print_capture() {
        let cap = MacOsCgCapture::new();
        match cap.capture_all_screens() {
            Ok(frames) => {
                eprintln!("[debug_print_capture] captured {} screens", frames.len());
                for f in &frames {
                    eprintln!(
                        "[debug_print_capture] screen {}: logical_bounds={:.0},{:.0} {:.0}x{:.0} image={}x{} dpi_scale={:.3}",
                        f.screen_id,
                        f.logical_bounds.x,
                        f.logical_bounds.y,
                        f.logical_bounds.w,
                        f.logical_bounds.h,
                        f.image.width(),
                        f.image.height(),
                        f.dpi_scale
                    );
                    let path = format!("/tmp/screenshot-raw-screen-{}.png", f.screen_id);
                    if let Err(e) = f.image.save(&path) {
                        eprintln!("[debug_print_capture] failed to save {}: {}", path, e);
                    } else {
                        eprintln!("[debug_print_capture] saved raw capture to {}", path);
                    }
                }
            }
            Err(e) => eprintln!("[debug_print_capture] capture failed: {:?}", e),
        }
    }
}
