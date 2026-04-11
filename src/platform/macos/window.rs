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
        let Some(dict) = array.get(i) else { continue };

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

        if layer < 0 {
            continue;
        }

        windows.push(DetectedWindow {
            id: id.to_string(),
            title,
            bounds: Rect::new(x, y, w, h),
            owner_pid: pid,
            z_order: layer,
        });
    }

    windows
}
