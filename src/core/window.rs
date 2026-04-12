use super::types::{DetectedWindow, LogicalPoint};
use rstar::{RTree, RTreeObject, AABB};

#[cfg(test)]
use super::types::Rect;

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
        // Keep all windows in the R-tree and rely on hit_test returning the
        // top-most (highest z_order) match. Geometric filtering is fragile
        // because front windows may be excluded from the list (e.g. system
        // windows, tiny windows), causing back windows to incorrectly survive.
        let items: Vec<WindowItem> = windows.into_iter().map(|w| WindowItem { window: w }).collect();
        Self { tree: RTree::bulk_load(items) }
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
        *self = Self::new(windows);
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

    #[test]
    fn hit_test_respects_true_paint_order() {
        let w1 = DetectedWindow {
            id: "w1".into(),
            title: "Bottom".into(),
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            owner_pid: 1,
            z_order: 0,
        };
        let w2 = DetectedWindow {
            id: "w2".into(),
            title: "Top".into(),
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            owner_pid: 2,
            z_order: 1,
        };
        let detector = WindowDetector::new(vec![w1, w2]);
        // w2 fully covers w1, so w1 should be filtered out by contains_rect
        let result = detector.hit_test(LogicalPoint::new(50.0, 50.0));
        assert_eq!(result.map(|w| w.id.as_str()), Some("w2"));
    }

    #[test]
    fn hit_test_union_occlusion_filters_fully_covered_window() {
        let w1 = DetectedWindow {
            id: "w1".into(),
            title: "Bottom".into(),
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            owner_pid: 1,
            z_order: 0,
        };
        let w2 = DetectedWindow {
            id: "w2".into(),
            title: "TopLeft".into(),
            bounds: Rect::new(0.0, 0.0, 50.0, 100.0),
            owner_pid: 2,
            z_order: 1,
        };
        let w3 = DetectedWindow {
            id: "w3".into(),
            title: "TopRight".into(),
            bounds: Rect::new(50.0, 0.0, 50.0, 100.0),
            owner_pid: 3,
            z_order: 2,
        };
        let detector = WindowDetector::new(vec![w1, w2, w3]);
        // w1 is fully covered by w2 + w3, so it should be filtered out
        assert_eq!(
            detector.hit_test(LogicalPoint::new(25.0, 50.0)).map(|w| w.id.as_str()),
            Some("w2")
        );
        assert_eq!(
            detector.hit_test(LogicalPoint::new(75.0, 50.0)).map(|w| w.id.as_str()),
            Some("w3")
        );
        // w1 should not be detectable anywhere
        assert_ne!(
            detector.hit_test(LogicalPoint::new(50.0, 50.0)).map(|w| w.id.as_str()),
            Some("w1")
        );
    }

    #[test]
    fn hit_test_partial_overlap_returns_top() {
        let w1 = DetectedWindow {
            id: "w1".into(),
            title: "Bottom".into(),
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            owner_pid: 1,
            z_order: 0,
        };
        let w2 = DetectedWindow {
            id: "w2".into(),
            title: "Top".into(),
            bounds: Rect::new(30.0, 30.0, 40.0, 40.0),
            owner_pid: 2,
            z_order: 1,
        };
        let detector = WindowDetector::new(vec![w1, w2]);
        // Inside overlap -> should return top (w2)
        let result = detector.hit_test(LogicalPoint::new(50.0, 50.0));
        assert_eq!(result.map(|w| w.id.as_str()), Some("w2"));

        // Outside w2 but still inside w1 -> should return w1
        let result = detector.hit_test(LogicalPoint::new(10.0, 10.0));
        assert_eq!(result.map(|w| w.id.as_str()), Some("w1"));
    }
}
