use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    pub fn area(&self) -> f64 {
        (self.w * self.h).max(0.0)
    }

    pub fn contains_rect(&self, other: Rect) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.x + self.w >= other.x + other.w
            && self.y + self.h >= other.y + other.h
    }

    pub fn intersects(&self, other: Rect) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }

    pub fn intersection(&self, other: Rect) -> Option<Rect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w);
        let y2 = (self.y + self.h).min(other.y + other.h);
        let w = x2 - x1;
        let h = y2 - y1;
        if w > 0.0 && h > 0.0 {
            Some(Rect::new(x1, y1, w, h))
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    pub fn hit_test_resize_handle(&self, p: LogicalPoint, handle_size: f64) -> Option<ResizeHit> {
        if self.is_empty() {
            return None;
        }
        let half = handle_size / 2.0;
        let outer_x = self.x - half;
        let outer_y = self.y - half;
        let outer_w = self.w + handle_size;
        let outer_h = self.h + handle_size;
        let outer = Rect::new(outer_x, outer_y, outer_w, outer_h);
        if !outer.contains(p) {
            return None;
        }

        // Corners (priority: corner > edge > interior)
        if p.x < self.x + half && p.y < self.y + half {
            return Some(ResizeHit::ResizeCorner { corner: Corner::NW });
        }
        if p.x >= self.x + self.w - half && p.y < self.y + half {
            return Some(ResizeHit::ResizeCorner { corner: Corner::NE });
        }
        if p.x < self.x + half && p.y >= self.y + self.h - half {
            return Some(ResizeHit::ResizeCorner { corner: Corner::SW });
        }
        if p.x >= self.x + self.w - half && p.y >= self.y + self.h - half {
            return Some(ResizeHit::ResizeCorner { corner: Corner::SE });
        }

        // Edges
        if p.y < self.y + half {
            return Some(ResizeHit::ResizeEdge { edge: Edge::North });
        }
        if p.y >= self.y + self.h - half {
            return Some(ResizeHit::ResizeEdge { edge: Edge::South });
        }
        if p.x < self.x + half {
            return Some(ResizeHit::ResizeEdge { edge: Edge::West });
        }
        if p.x >= self.x + self.w - half {
            return Some(ResizeHit::ResizeEdge { edge: Edge::East });
        }

        // Interior move area
        if self.contains(p) {
            return Some(ResizeHit::Move);
        }

        None
    }
}

/// Subtract `cut` from `target`, returning the remaining non-overlapping rectangles.
pub fn subtract_rect(target: Rect, cut: Rect) -> Vec<Rect> {
    if !target.intersects(cut) {
        return vec![target];
    }
    if cut.contains_rect(target) {
        return vec![];
    }

    let mut result = Vec::new();

    // Top strip
    if cut.y > target.y {
        result.push(Rect::new(target.x, target.y, target.w, cut.y - target.y));
    }

    // Bottom strip
    if cut.y + cut.h < target.y + target.h {
        result.push(Rect::new(
            target.x,
            cut.y + cut.h,
            target.w,
            target.y + target.h - (cut.y + cut.h),
        ));
    }

    let y_overlap_top = target.y.max(cut.y);
    let y_overlap_bottom = (target.y + target.h).min(cut.y + cut.h);

    // Left strip (within the vertical overlap)
    if cut.x > target.x && y_overlap_bottom > y_overlap_top {
        result.push(Rect::new(
            target.x,
            y_overlap_top,
            cut.x - target.x,
            y_overlap_bottom - y_overlap_top,
        ));
    }

    // Right strip (within the vertical overlap)
    if cut.x + cut.w < target.x + target.w && y_overlap_bottom > y_overlap_top {
        result.push(Rect::new(
            cut.x + cut.w,
            y_overlap_top,
            target.x + target.w - (cut.x + cut.w),
            y_overlap_bottom - y_overlap_top,
        ));
    }

    result
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenInfo {
    pub id: String,
    pub name: String,
    pub logical_bounds: Rect,
    pub dpi_scale: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Edge {
    North,
    South,
    East,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Corner {
    NW,
    NE,
    SW,
    SE,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ResizeHit {
    Move,
    ResizeEdge { edge: Edge },
    ResizeCorner { corner: Corner },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    #[test]
    fn rect_intersection() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let inter = a.intersection(b).unwrap();
        assert_eq!(inter.x, 50.0);
        assert_eq!(inter.y, 50.0);
        assert_eq!(inter.w, 50.0);
        assert_eq!(inter.h, 50.0);
    }

    #[test]
    fn rect_intersection_none() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert!(a.intersection(b).is_none());
    }

    #[test]
    fn resize_hit_center_is_move() {
        let r = Rect::new(100.0, 100.0, 100.0, 100.0);
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(150.0, 150.0), 20.0),
            Some(ResizeHit::Move)
        );
    }

    #[test]
    fn resize_hit_corners() {
        let r = Rect::new(100.0, 100.0, 100.0, 100.0);
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(105.0, 105.0), 20.0),
            Some(ResizeHit::ResizeCorner { corner: Corner::NW })
        );
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(195.0, 105.0), 20.0),
            Some(ResizeHit::ResizeCorner { corner: Corner::NE })
        );
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(105.0, 195.0), 20.0),
            Some(ResizeHit::ResizeCorner { corner: Corner::SW })
        );
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(195.0, 195.0), 20.0),
            Some(ResizeHit::ResizeCorner { corner: Corner::SE })
        );
    }

    #[test]
    fn resize_hit_edges() {
        let r = Rect::new(100.0, 100.0, 100.0, 100.0);
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(150.0, 105.0), 20.0),
            Some(ResizeHit::ResizeEdge { edge: Edge::North })
        );
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(150.0, 195.0), 20.0),
            Some(ResizeHit::ResizeEdge { edge: Edge::South })
        );
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(105.0, 150.0), 20.0),
            Some(ResizeHit::ResizeEdge { edge: Edge::West })
        );
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(195.0, 150.0), 20.0),
            Some(ResizeHit::ResizeEdge { edge: Edge::East })
        );
    }

    #[test]
    fn resize_hit_outside_is_none() {
        let r = Rect::new(100.0, 100.0, 100.0, 100.0);
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(0.0, 0.0), 20.0),
            None
        );
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(250.0, 150.0), 20.0),
            None
        );
    }

    #[test]
    fn resize_hit_empty_rect_is_none() {
        let r = Rect::new(100.0, 100.0, 0.0, 50.0);
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(100.0, 125.0), 20.0),
            None
        );
    }

    #[test]
    fn resize_hit_boundary_between_zones() {
        let r = Rect::new(100.0, 100.0, 100.0, 100.0);
        let half = 10.0;
        // Exactly at inner corner threshold; code uses < so it falls through to Move
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(r.x + half, r.y + half), 20.0),
            Some(ResizeHit::Move)
        );
    }

    #[test]
    fn resize_hit_pure_edge_away_from_corners() {
        let r = Rect::new(100.0, 100.0, 100.0, 100.0);
        let half = 10.0;
        let x = r.x - half + 1.0;
        let y = r.y + r.h / 2.0;
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(x, y), 20.0),
            Some(ResizeHit::ResizeEdge { edge: Edge::West })
        );
    }

    #[test]
    fn resize_hit_handle_size_zero() {
        let r = Rect::new(100.0, 100.0, 100.0, 100.0);
        // Strictly inside -> Move
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(150.0, 150.0), 0.0),
            Some(ResizeHit::Move)
        );
        // Exactly on lower border -> inside due to contains lower-bound inclusivity, so Move
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(100.0, 100.0), 0.0),
            Some(ResizeHit::Move)
        );
        // Exactly on upper x border -> outside due to contains upper-bound exclusivity, so None
        assert_eq!(
            r.hit_test_resize_handle(LogicalPoint::new(200.0, 150.0), 0.0),
            None
        );
    }
}
