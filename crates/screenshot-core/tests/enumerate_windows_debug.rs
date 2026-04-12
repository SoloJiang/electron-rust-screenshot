use screenshot_core::platform::macos::window::enumerate_windows;

#[test]
fn debug_enumerate_windows() {
    let windows = enumerate_windows();
    eprintln!("Total windows: {}", windows.len());
    for (i, w) in windows.iter().rev().take(10).enumerate() {
        eprintln!(
            "front #{}: z_order={}, title={:?}, bounds={:?}",
            i, w.z_order, w.title, w.bounds
        );
    }
    for (i, w) in windows.iter().take(5).enumerate() {
        eprintln!(
            "back #{}: z_order={}, title={:?}, bounds={:?}",
            i, w.z_order, w.title, w.bounds
        );
    }
}
