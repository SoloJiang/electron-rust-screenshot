use crate::core::types::{LogicalPoint, ScreenInfo};

/// Convert a logical point in engine-global coords to physical screen-pixel
/// coords usable by cliclick / SendInput.
pub fn logical_to_physical(point: LogicalPoint, screens: &[ScreenInfo]) -> Option<(i32, i32)> {
    let screen = screens.iter().find(|s| s.logical_bounds.contains(point))?;
    let local_x = point.x - screen.logical_bounds.x;
    let local_y = point.y - screen.logical_bounds.y;
    let phys_x = screen.physical_origin.0 as f64 + local_x * screen.dpi_scale;
    let phys_y = screen.physical_origin.1 as f64 + local_y * screen.dpi_scale;
    Some((phys_x.round() as i32, phys_y.round() as i32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{LogicalPoint, Rect, ScreenInfo};

    fn screens_2x_retina() -> Vec<ScreenInfo> {
        vec![ScreenInfo {
            id: "1".into(),
            name: "Built-in".into(),
            logical_bounds: Rect::new(0.0, 0.0, 1440.0, 900.0),
            dpi_scale: 2.0,
            physical_origin: (0, 0),
            is_primary: true,
        }]
    }

    fn screens_dual() -> Vec<ScreenInfo> {
        vec![
            ScreenInfo {
                id: "1".into(),
                name: "Built-in".into(),
                logical_bounds: Rect::new(0.0, 0.0, 1440.0, 900.0),
                dpi_scale: 2.0,
                physical_origin: (0, 0),
                is_primary: true,
            },
            ScreenInfo {
                id: "2".into(),
                name: "External".into(),
                logical_bounds: Rect::new(1440.0, 0.0, 1920.0, 1080.0),
                dpi_scale: 1.0,
                physical_origin: (2880, 0),
                is_primary: false,
            },
        ]
    }

    #[test]
    fn maps_origin_to_origin_on_2x_display() {
        let p = logical_to_physical(LogicalPoint::new(0.0, 0.0), &screens_2x_retina()).unwrap();
        assert_eq!(p, (0, 0));
    }

    #[test]
    fn maps_50_50_logical_to_100_100_physical_on_2x() {
        let p = logical_to_physical(LogicalPoint::new(50.0, 50.0), &screens_2x_retina()).unwrap();
        assert_eq!(p, (100, 100));
    }

    #[test]
    fn maps_secondary_display_with_offset() {
        // (1500, 100) is on display 2 → local (60, 100), 1x scale, origin (2880,0)
        let p = logical_to_physical(LogicalPoint::new(1500.0, 100.0), &screens_dual()).unwrap();
        assert_eq!(p, (2940, 100));
    }

    #[test]
    fn returns_none_outside_all_displays() {
        let p = logical_to_physical(LogicalPoint::new(99999.0, 0.0), &screens_dual());
        assert!(p.is_none());
    }
}
