use image::GenericImageView;

pub fn check(path: &str, expected_width: u32, expected_height: u32) -> Result<(), String> {
    let img = image::open(path).map_err(|e| format!("dimensions: failed to open '{path}': {e}"))?;
    let (w, h) = img.dimensions();
    if w != expected_width || h != expected_height {
        Err(format!(
            "dimensions: expected {expected_width}x{expected_height}, got {w}x{h}"
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_png(w: u32, h: u32, path: &str) {
        let img = image::RgbaImage::new(w, h);
        img.save(path).unwrap();
    }

    #[test]
    fn detects_correct_dimensions() {
        let path = "/tmp/validator_test_dim_ok.png";
        make_test_png(100, 200, path);
        assert!(check(path, 100, 200).is_ok());
    }

    #[test]
    fn rejects_wrong_dimensions() {
        let path = "/tmp/validator_test_dim_fail.png";
        make_test_png(50, 50, path);
        let err = check(path, 100, 200).unwrap_err();
        assert!(err.contains("expected 100x200, got 50x50"));
    }
}
