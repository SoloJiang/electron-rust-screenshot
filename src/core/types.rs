#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, p: LogicalPoint) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }

    pub fn area(&self) -> f64 {
        (self.w * self.h).max(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalPoint {
    pub x: f64,
    pub y: f64,
}

impl LogicalPoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_sq(&self, other: LogicalPoint) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenInfo {
    pub id: String,
    pub name: String,
    pub logical_bounds: Rect,
    pub dpi_scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedWindow {
    pub id: String,
    pub title: String,
    pub bounds: Rect,
    pub owner_pid: i64,
    pub z_order: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_point() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(r.contains(LogicalPoint::new(50.0, 50.0)));
        assert!(!r.contains(LogicalPoint::new(150.0, 50.0)));
    }

    #[test]
    fn point_distance() {
        let a = LogicalPoint::new(0.0, 0.0);
        let b = LogicalPoint::new(3.0, 4.0);
        assert_eq!(a.distance_sq(b), 25.0);
    }
}
