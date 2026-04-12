use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub fn setup_window(window: &Window) {
    unsafe {
        use objc::class;
        use objc::msg_send;
        use objc::runtime::Object;
        use objc::sel;
        use objc::sel_impl;
        let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let policy: i64 = 0; // NSApplicationActivationPolicyRegular
        let _: () = msg_send![ns_app, setActivationPolicy: policy];
        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                let ns_view: *mut Object = appkit.ns_view.as_ptr() as *mut Object;
                let ns_window: *mut Object = msg_send![ns_view, window];
                if !ns_window.is_null() {
                    let level: i64 = 25; // NSStatusWindowLevel
                    let _: () = msg_send![ns_window, setLevel: level];
                    let _: () = msg_send![ns_window, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
                }
            }
        }
    }
}

pub fn set_mouse_passthrough(window: &Window, ignores: bool) {
    unsafe {
        use objc::msg_send;
        use objc::runtime::Object;
        use objc::sel;
        use objc::sel_impl;
        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                let ns_view: *mut Object = appkit.ns_view.as_ptr() as *mut Object;
                let ns_window: *mut Object = msg_send![ns_view, window];
                if !ns_window.is_null() {
                    let _: () = msg_send![ns_window, setIgnoresMouseEvents: ignores];
                }
            }
        }
    }
}
