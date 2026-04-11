use super::types::{DetectedWindow, LogicalPoint, Rect};
use rstar::{RTree, RTreeObject, AABB};

struct WindowItem {
    window: DetectedWindow,
}

impl RTreeObject for WindowItem {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.window.bounds.x, self.window.bounds.y],
            [
                self.window.bounds.x + self.window.bounds.w,
                self.window.bounds.y + self.window.bounds.h,
            ],
        )
    }
}

pub struct WindowDetector {
    tree: RTree<WindowItem>,
}

impl WindowDetector {
    pub fn new(windows: Vec<DetectedWindow>) -> Self {
        let items: Vec<WindowItem> = windows
            .into_iter()
            .map(|w| WindowItem { window: w })
            .collect();
        Self {
            tree: RTree::bulk_load(items),
        }
    }

    pub fn hit_test(&self, point: LogicalPoint) -> Option<&DetectedWindow> {
        let results: Vec<&WindowItem> = self
            .tree
            .locate_in_envelope_intersecting(&AABB::from_point([point.x, point.y]))
            .collect();
        // Return top-most (highest z_order)
        results
            .into_iter()
            .max_by_key(|item| item.window.z_order)
            .map(|item| &item.window)
    }

    pub fn update_windows(&mut self, windows: Vec<DetectedWindow>) {
        let items: Vec<WindowItem> = windows
            .into_iter()
            .map(|w| WindowItem { window: w })
            .collect();
        self.tree = RTree::bulk_load(items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_returns_top_window() {
        let w1 = DetectedWindow {
            id: "w1".into(),
            title: "A".into(),
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            owner_pid: 1,
            z_order: 1,
        };
        let w2 = DetectedWindow {
            id: "w2".into(),
            title: "B".into(),
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            owner_pid: 2,
            z_order: 10,
        };
        let detector = WindowDetector::new(vec![w1, w2]);
        let result = detector.hit_test(LogicalPoint::new(50.0, 50.0));
        assert_eq!(result.map(|w| w.z_order), Some(10));
    }

    #[test]
    fn hit_test_miss() {
        let w1 = DetectedWindow {
            id: "w1".into(),
            title: "A".into(),
            bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
            owner_pid: 1,
            z_order: 1,
        };
        let detector = WindowDetector::new(vec![w1]);
        assert!(detector.hit_test(LogicalPoint::new(100.0, 100.0)).is_none());
    }
}
