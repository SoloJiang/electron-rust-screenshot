use crate::core::types::{DetectedWindow, Rect};
use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::dictionary::{CFDictionary, CFDictionaryGetValueIfPresent};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::display::{
    kCGNullWindowID, kCGWindowListOptionOnScreenOnly, CGWindowListCopyWindowInfo,
};
use std::ffi::c_void;

fn get_i64(dict: &CFDictionary, key: &str) -> Option<i64> {
    let key = CFString::new(key);
    let mut value: *const c_void = std::ptr::null();
    let found = unsafe {
        CFDictionaryGetValueIfPresent(
            dict.as_concrete_TypeRef(),
            key.as_concrete_TypeRef() as *const c_void,
            &mut value,
        )
    };
    if found != 0 && !value.is_null() {
        unsafe { CFNumber::wrap_under_get_rule(value as *mut _).to_i64() }
    } else {
        None
    }
}

fn get_f64(dict: &CFDictionary, key: &str) -> Option<f64> {
    let key = CFString::new(key);
    let mut value: *const c_void = std::ptr::null();
    let found = unsafe {
        CFDictionaryGetValueIfPresent(
            dict.as_concrete_TypeRef(),
            key.as_concrete_TypeRef() as *const c_void,
            &mut value,
        )
    };
    if found != 0 && !value.is_null() {
        unsafe { CFNumber::wrap_under_get_rule(value as *mut _).to_f64() }
    } else {
        None
    }
}

fn get_string(dict: &CFDictionary, key: &str) -> Option<String> {
    let key = CFString::new(key);
    let mut value: *const c_void = std::ptr::null();
    let found = unsafe {
        CFDictionaryGetValueIfPresent(
            dict.as_concrete_TypeRef(),
            key.as_concrete_TypeRef() as *const c_void,
            &mut value,
        )
    };
    if found != 0 && !value.is_null() {
        Some(unsafe { CFString::wrap_under_get_rule(value as *mut _).to_string() })
    } else {
        None
    }
}

pub fn enumerate_windows() -> Vec<DetectedWindow> {
    let options = kCGWindowListOptionOnScreenOnly;
    let window_list = unsafe { CGWindowListCopyWindowInfo(options, kCGNullWindowID) };
    let array = unsafe { CFArray::<CFDictionary>::wrap_under_create_rule(window_list) };
    let mut windows = Vec::new();

    for i in 0..array.len() {
        let idx = array.len() - 1 - i;
        let Some(dict) = array.get(idx) else { continue };

        let (x, y, w, h) = {
            let mut bounds_ptr: *const c_void = std::ptr::null();
            let bounds_found = unsafe {
                CFDictionaryGetValueIfPresent(
                    dict.as_concrete_TypeRef(),
                    CFString::new("kCGWindowBounds").as_concrete_TypeRef() as *const c_void,
                    &mut bounds_ptr,
                )
            };
            if bounds_found != 0 && !bounds_ptr.is_null() {
                let bounds = unsafe { CFDictionary::wrap_under_get_rule(bounds_ptr as *mut _) };
                let bx = get_i64(&bounds, "X").unwrap_or(0) as f64;
                let by = get_i64(&bounds, "Y").unwrap_or(0) as f64;
                let bw = get_i64(&bounds, "Width").unwrap_or(0) as f64;
                let bh = get_i64(&bounds, "Height").unwrap_or(0) as f64;
                (bx, by, bw, bh)
            } else {
                (0.0, 0.0, 0.0, 0.0)
            }
        };

        let id = get_i64(&dict, "kCGWindowNumber").unwrap_or(0);
        let pid = get_i64(&dict, "kCGWindowOwnerPID").unwrap_or(0);
        let title = get_string(&dict, "kCGWindowName").unwrap_or_default();
        let layer = get_i64(&dict, "kCGWindowLayer").unwrap_or(0) as i32;
        let alpha = get_f64(&dict, "kCGWindowAlpha").unwrap_or(1.0);

        if layer < 0 || alpha < 0.001 || w <= 1.0 || h <= 1.0 {
            continue;
        }

        windows.push(DetectedWindow {
            id: id.to_string(),
            title,
            bounds: Rect::new(x, y, w, h),
            owner_pid: pid,
            z_order: i as i32, // i=0 is back-most, i=N-1 is front-most
        });
    }

    // Debug: print first few windows to verify order
    for (i, w) in windows.iter().rev().take(5).enumerate() {
        eprintln!("[enumerate_windows] index {} (from front): z={}, title={:?}, bounds={:?}", i, w.z_order, w.title, w.bounds);
    }

    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn debug_print_enumerate_order() {
        let windows = enumerate_windows();
        eprintln!("Total windows returned: {}", windows.len());
        for (i, w) in windows.iter().rev().take(5).enumerate() {
            eprintln!(
                "frontmost #{}: z_order={} title={:?} bounds={:?}",
                i, w.z_order, w.title, w.bounds
            );
        }
        for (i, w) in windows.iter().take(5).enumerate() {
            eprintln!(
                "backmost #{}: z_order={} title={:?} bounds={:?}",
                i, w.z_order, w.title, w.bounds
            );
        }
    }

}
