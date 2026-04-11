use crate::core::capture::ScreenFrame;
use crate::core::editor::{EditorState, Layer};
use crate::core::types::LogicalPoint;
use image::{ImageFormat, Rgba, RgbaImage};
use std::path::Path;

pub fn composite_and_save(
    frames: &[ScreenFrame],
    editor: &EditorState,
    save_path: &str,
    format: &str,
    _quality: u8,
) -> Result<String, String> {
    let selection = editor.selection.ok_or("No selection")?;
    let frame = frames
        .iter()
        .find(|f| {
            f.logical_bounds.contains(LogicalPoint::new(selection.x, selection.y))
        })
        .ok_or("No frame for selection")?;

    let physical_rect = crate::core::dpi::rect_logical_to_physical(selection, frame.dpi_scale);
    let x = physical_rect.x as u32;
    let y = physical_rect.y as u32;
    let w = physical_rect.w as u32;
    let h = physical_rect.h as u32;

    let source = &frame.image;
    if x + w > source.width() || y + h > source.height() {
        return Err("Selection out of bounds".into());
    }

    let mut output = image::imageops::crop_imm(source, x, y, w, h).to_image();

    for layer in &editor.layers {
        match layer {
            Layer::MosaicPath { points, block_size, .. } => {
                apply_mosaic(&mut output, points, *block_size, frame.dpi_scale);
            }
            _ => {}
        }
    }

    let path = Path::new(save_path);
    let format_enum = match format {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::WebP,
        _ => ImageFormat::Png,
    };

    output
        .save_with_format(path, format_enum)
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn apply_mosaic(
    img: &mut RgbaImage,
    _points: &[LogicalPoint],
    block_size: f32,
    scale: f64,
) {
    let bs = (block_size * scale as f32) as u32;
    if bs == 0 {
        return;
    }
    let (w, h) = (img.width(), img.height());
    for by in (0..h).step_by(bs as usize) {
        for bx in (0..w).step_by(bs as usize) {
            let end_x = (bx + bs).min(w);
            let end_y = (by + bs).min(h);
            let mut r = 0u32;
            let mut g = 0u32;
            let mut b = 0u32;
            let mut count = 0u32;
            for y in by..end_y {
                for x in bx..end_x {
                    let p = img.get_pixel(x, y);
                    r += p[0] as u32;
                    g += p[1] as u32;
                    b += p[2] as u32;
                    count += 1;
                }
            }
            if count > 0 {
                let color = Rgba([(r / count) as u8, (g / count) as u8, (b / count) as u8, 255]);
                for y in by..end_y {
                    for x in bx..end_x {
                        img.put_pixel(x, y, color);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Color, Rect};

    #[test]
    fn composite_with_no_layers() {
        let img = RgbaImage::new(100, 100);
        let frames = vec![ScreenFrame {
            screen_id: "1".into(),
            logical_bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            dpi_scale: 1.0,
            image: img,
        }];
        let mut editor = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
        editor.selection = Some(Rect::new(0.0, 0.0, 50.0, 50.0));
        let path = composite_and_save(&frames, &editor, "/tmp/test-composite.png", "png", 90);
        assert!(path.is_ok());
    }
}
