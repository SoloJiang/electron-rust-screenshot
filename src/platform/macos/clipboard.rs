use cocoa::appkit::NSPasteboard;
use cocoa::base::nil;
use cocoa::foundation::NSData;
use image::RgbaImage;
use objc::runtime::{Class, Object};
use objc::{class, msg_send, sel, sel_impl};

pub fn copy_image_to_clipboard(img: &RgbaImage) -> Result<(), String> {
    unsafe {
        let pasteboard = NSPasteboard::generalPasteboard(nil);
        pasteboard.clearContents();

        let width = img.width();
        let height = img.height();
        let raw: Vec<u8> = img.pixels().flat_map(|p| p.0.to_vec()).collect();

        let ns_data = NSData::dataWithBytes_length_(
            nil,
            raw.as_ptr() as *const std::ffi::c_void,
            raw.len() as u64,
        );

        let ns_image_class = Class::get("NSImage").ok_or("NSImage not found")?;
        let ns_image: *mut Object = msg_send![ns_image_class, alloc];
        let ns_image: *mut Object = msg_send![ns_image, initWithData: ns_data];

        if ns_image.is_null() {
            return Err("Failed to create NSImage".into());
        }

        let _: () = msg_send![ns_image, setSize: (width as f64, height as f64)];
        let objects: *mut Object = msg_send![class!(NSArray), arrayWithObject: ns_image];
        let _: i32 = msg_send![pasteboard, writeObjects: objects];

        Ok(())
    }
}
