use super::types::{LogicalPoint, Rect};

pub fn physical_to_logical(px: f64, scale: f64) -> f64 {
    px / scale
}

pub fn logical_to_physical(px: f64, scale: f64) -> f64 {
    px * scale
}

pub fn rect_physical_to_logical(r: Rect, scale: f64) -> Rect {
    Rect::new(
        r.x / scale,
        r.y / scale,
        r.w / scale,
        r.h / scale,
    )
}

pub fn rect_logical_to_physical(r: Rect, scale: f64) -> Rect {
    Rect::new(
        r.x * scale,
        r.y * scale,
        r.w * scale,
        r.h * scale,
    )
}

pub fn point_physical_to_logical(p: LogicalPoint, scale: f64) -> LogicalPoint {
    LogicalPoint::new(p.x / scale, p.y / scale)
}

pub fn point_logical_to_physical(p: LogicalPoint, scale: f64) -> LogicalPoint {
    LogicalPoint::new(p.x * scale, p.y * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_conversion() {
        assert_eq!(physical_to_logical(200.0, 2.0), 100.0);
        assert_eq!(logical_to_physical(100.0, 2.0), 200.0);
    }

    #[test]
    fn rect_conversion_roundtrip() {
        let r = Rect::new(100.0, 200.0, 300.0, 400.0);
        let logical = rect_physical_to_logical(r, 2.0);
        let physical = rect_logical_to_physical(logical, 2.0);
        assert_eq!(r, physical);
    }
}
