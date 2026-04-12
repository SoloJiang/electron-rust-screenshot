use crate::core::capture::ScreenFrame;
use crate::core::editor::{EditorState, Layer};
use crate::core::types::{LogicalPoint, Rect};
use image::{ImageFormat, Rgba, RgbaImage};
use std::path::Path;

fn intersect_rects(a: Rect, b: Rect) -> Option<Rect> {
    a.intersection(b)
}

fn global_to_local(r: Rect, bounds: Rect) -> Rect {
    Rect::new(r.x - bounds.x, r.y - bounds.y, r.w, r.h)
}

pub fn composite_image(frames: &[ScreenFrame], editor: &EditorState) -> Result<RgbaImage, String> {
    let selection = editor.selection.ok_or("No selection")?;

    let intersecting: Vec<&ScreenFrame> = frames
        .iter()
        .filter(|f| f.logical_bounds.intersects(selection))
        .collect();

    if intersecting.is_empty() {
        return Err("No frame for selection".into());
    }

    // Determine dominant DPI from the screen that contains the selection center.
    let cx = selection.x + selection.w / 2.0;
    let cy = selection.y + selection.h / 2.0;
    let dominant_dpi = frames
        .iter()
        .find(|f| f.logical_bounds.contains(LogicalPoint::new(cx, cy)))
        .map(|f| f.dpi_scale)
        .unwrap_or(1.0);

    // Compute union of all clipped intersections in global logical space.
    let union_rect = {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        for frame in &intersecting {
            if let Some(clip) = intersect_rects(selection, frame.logical_bounds) {
                min_x = min_x.min(clip.x);
                min_y = min_y.min(clip.y);
                max_x = max_x.max(clip.x + clip.w);
                max_y = max_y.max(clip.y + clip.h);
            }
        }
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    };

    let out_w = (union_rect.w * dominant_dpi).ceil().max(0.0) as u32;
    let out_h = (union_rect.h * dominant_dpi).ceil().max(0.0) as u32;
    let mut output = RgbaImage::new(out_w, out_h);

    for frame in intersecting {
        let Some(overlap) = intersect_rects(selection, frame.logical_bounds) else {
            continue;
        };
        let local_logical = global_to_local(overlap, frame.logical_bounds);
        let physical = crate::core::dpi::rect_logical_to_physical(local_logical, frame.dpi_scale);
        let x = physical.x.max(0.0) as u32;
        let y = physical.y.max(0.0) as u32;
        let w = physical.w.max(0.0) as u32;
        let h = physical.h.max(0.0) as u32;

        if x + w > frame.image.width() || y + h > frame.image.height() {
            return Err("Selection out of bounds".into());
        }

        let cropped = image::imageops::crop_imm(&frame.image, x, y, w, h).to_image();

        let offset_in_union = global_to_local(overlap, union_rect);
        let output_offset = crate::core::dpi::rect_logical_to_physical(offset_in_union, dominant_dpi);
        let ox = output_offset.x as i64;
        let oy = output_offset.y as i64;

        // If DPIs differ, scale the cropped piece to fit into the dominant-dpi canvas.
        if (frame.dpi_scale - dominant_dpi).abs() > f64::EPSILON {
            let target_w = (overlap.w * dominant_dpi).ceil().max(0.0) as u32;
            let target_h = (overlap.h * dominant_dpi).ceil().max(0.0) as u32;
            let scaled = image::imageops::resize(
                &cropped,
                target_w,
                target_h,
                image::imageops::FilterType::Lanczos3,
            );
            image::imageops::overlay(&mut output, &scaled, ox, oy);
        } else {
            image::imageops::overlay(&mut output, &cropped, ox, oy);
        }
    }

    for layer in &editor.layers {
        if let Layer::MosaicPath {
            points, block_size, ..
        } = layer
        {
            apply_mosaic(&mut output, points, *block_size, dominant_dpi);
        }
    }

    Ok(output)
}

pub fn composite_and_save(
    frames: &[ScreenFrame],
    editor: &EditorState,
    save_path: &str,
    format: &str,
    _quality: u8,
) -> Result<String, String> {
    let output = composite_image(frames, editor)?;

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

fn apply_mosaic(img: &mut RgbaImage, _points: &[LogicalPoint], block_size: f32, scale: f64) {
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
            if let Some(color) = r
                .checked_div(count)
                .zip(g.checked_div(count))
                .zip(b.checked_div(count))
                .map(|((rr, gg), bb)| Rgba([rr as u8, gg as u8, bb as u8, 255]))
            {
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

    #[test]
    fn composite_cross_screen_stitches_correctly() {
        let mut img1 = RgbaImage::new(100, 100);
        for p in img1.pixels_mut() {
            *p = Rgba([255, 0, 0, 255]);
        }
        let mut img2 = RgbaImage::new(100, 100);
        for p in img2.pixels_mut() {
            *p = Rgba([0, 0, 255, 255]);
        }
        let frames = vec![
            ScreenFrame {
                screen_id: "left".into(),
                logical_bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
                dpi_scale: 1.0,
                image: img1,
            },
            ScreenFrame {
                screen_id: "right".into(),
                logical_bounds: Rect::new(100.0, 0.0, 100.0, 100.0),
                dpi_scale: 1.0,
                image: img2,
            },
        ];
        let mut editor = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
        // selection straddles the boundary
        editor.selection = Some(Rect::new(50.0, 25.0, 100.0, 50.0));
        let out = composite_image(&frames, &editor).unwrap();
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 50);
        // left half should be red, right half blue
        assert_eq!(*out.get_pixel(10, 10), Rgba([255, 0, 0, 255]));
        assert_eq!(*out.get_pixel(60, 10), Rgba([0, 0, 255, 255]));
    }
}
