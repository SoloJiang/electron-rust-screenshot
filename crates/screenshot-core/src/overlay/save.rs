use crate::core::capture::ScreenFrame;
use crate::core::editor::{EditorState, Layer};
use crate::core::types::{LogicalPoint, Rect};
use image::{ImageFormat, Rgba, RgbaImage};
use std::path::Path;

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
            if let Some(clip) = selection.intersection(frame.logical_bounds) {
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
        let Some(overlap) = selection.intersection(frame.logical_bounds) else {
            continue;
        };
        let local_logical = global_to_local(overlap, frame.logical_bounds);
        let physical = crate::core::dpi::rect_logical_to_physical(local_logical, frame.dpi_scale);
        let mut x = physical.x.max(0.0) as u32;
        let mut y = physical.y.max(0.0) as u32;
        let mut w = physical.w.max(0.0) as u32;
        let mut h = physical.h.max(0.0) as u32;

        // Clamp to source image bounds instead of erroring.
        let img_w = frame.image.width();
        let img_h = frame.image.height();
        if x > img_w {
            x = img_w;
        }
        if y > img_h {
            y = img_h;
        }
        if x + w > img_w {
            w = img_w - x;
        }
        if y + h > img_h {
            h = img_h - y;
        }

        if w == 0 || h == 0 {
            continue;
        }

        let cropped = image::imageops::crop_imm(&frame.image, x, y, w, h).to_image();

        let offset_in_union = global_to_local(overlap, union_rect);
        let output_offset =
            crate::core::dpi::rect_logical_to_physical(offset_in_union, dominant_dpi);
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

fn apply_mosaic(img: &mut RgbaImage, points: &[LogicalPoint], block_size: f32, scale: f64) {
    if points.is_empty() {
        return;
    }
    let bs = (block_size * scale as f32) as u32;
    if bs == 0 {
        return;
    }

    // Compute bounding box of the brush path in logical space,
    // then convert to physical pixels on the composite image.
    let mut min_x = points[0].x;
    let mut min_y = points[0].y;
    let mut max_x = points[0].x;
    let mut max_y = points[0].y;
    for p in &points[1..] {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    // Add a small margin so stroke edges are covered.
    let margin = block_size as f64;
    let path_bounds_logical = Rect::new(
        min_x - margin,
        min_y - margin,
        (max_x - min_x) + margin * 2.0,
        (max_y - min_y) + margin * 2.0,
    );
    let path_bounds = crate::core::dpi::rect_logical_to_physical(path_bounds_logical, scale);

    let (img_w, img_h) = (img.width(), img.height());
    let bounds_x = path_bounds.x.max(0.0) as u32;
    let bounds_y = path_bounds.y.max(0.0) as u32;
    let bounds_x2 = (path_bounds.x + path_bounds.w).min(img_w as f64) as u32;
    let bounds_y2 = (path_bounds.y + path_bounds.h).min(img_h as f64) as u32;

    for by in (bounds_y..bounds_y2).step_by(bs as usize) {
        for bx in (bounds_x..bounds_x2).step_by(bs as usize) {
            let end_x = (bx + bs).min(bounds_x2);
            let end_y = (by + bs).min(bounds_y2);
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
