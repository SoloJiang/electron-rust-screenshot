pub fn check(path: &str, against: &str, max_diff_ratio: f64) -> Result<(), String> {
    let img_a =
        image::open(path).map_err(|e| format!("pixel_diff: failed to open '{path}': {e}"))?;
    let img_b =
        image::open(against).map_err(|e| format!("pixel_diff: failed to open '{against}': {e}"))?;

    let a = img_a.to_rgba8();
    let b = img_b.to_rgba8();

    if a.dimensions() != b.dimensions() {
        return Err(format!(
            "pixel_diff: dimension mismatch {path} ({:?}) vs {against} ({:?})",
            a.dimensions(),
            b.dimensions()
        ));
    }

    let (w, h) = a.dimensions();
    let total_pixels = (w * h) as usize;
    let mut diff_pixels = 0usize;

    for (pa, pb) in a.pixels().zip(b.pixels()) {
        if pa.0 != pb.0 {
            diff_pixels += 1;
        }
    }

    let ratio = diff_pixels as f64 / total_pixels as f64;
    if ratio > max_diff_ratio {
        Err(format!(
            "pixel_diff: diff ratio {ratio:.4} ({diff_pixels}/{total_pixels}) exceeds threshold {max_diff_ratio}"
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_png(w: u32, h: u32, fill: [u8; 4], path: &str) {
        let mut img = image::RgbaImage::new(w, h);
        for p in img.pixels_mut() {
            p.0 = fill;
        }
        img.save(path).unwrap();
    }

    #[test]
    fn identical_images_pass() {
        let a = "/tmp/validator_test_pixel_a.png";
        let b = "/tmp/validator_test_pixel_b.png";
        make_test_png(10, 10, [255, 0, 0, 255], a);
        make_test_png(10, 10, [255, 0, 0, 255], b);
        assert!(check(a, b, 0.0).is_ok());
    }

    #[test]
    fn slightly_different_within_threshold() {
        let a = "/tmp/validator_test_pixel_c.png";
        let b = "/tmp/validator_test_pixel_d.png";
        make_test_png(10, 10, [255, 0, 0, 255], a);
        make_test_png(10, 10, [255, 0, 0, 255], b);
        // flip one pixel in b
        let mut img = image::open(b).unwrap().to_rgba8();
        img.put_pixel(0, 0, image::Rgba([0, 255, 0, 255]));
        img.save(b).unwrap();
        // 1 diff out of 100 = 0.01
        assert!(check(a, b, 0.05).is_ok());
    }

    #[test]
    fn too_different_fails() {
        let a = "/tmp/validator_test_pixel_e.png";
        let b = "/tmp/validator_test_pixel_f.png";
        make_test_png(10, 10, [255, 0, 0, 255], a);
        make_test_png(10, 10, [0, 255, 0, 255], b);
        assert!(check(a, b, 0.01).is_err());
    }
}
