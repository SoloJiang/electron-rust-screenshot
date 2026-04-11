#![deny(clippy::all)]

pub mod bridge;
pub mod core;
pub mod overlay;
pub mod platform;

use crate::bridge::config::ScreenshotConfig;
use crate::bridge::session::ScreenshotSession;
use crate::core::engine::Engine;
use crate::core::types::Color;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::{Arc, Mutex};
use std::thread;

#[napi]
pub fn start(config: Option<ScreenshotConfig>) -> Result<ScreenshotSession> {
    let merged = config.map(|c| c.merge()).unwrap_or_default();

    let color = parse_color(&merged.default_color.unwrap_or_else(|| "#ff0000".into()));
    let save_path = merged.save_path.unwrap_or_else(|| "/tmp".into());
    let format = merged.format.unwrap_or_else(|| "png".into());
    let quality = merged.quality.unwrap_or(90) as u8;
    let size = merged.default_size.unwrap_or(3) as f32;
    let mosaic = merged.mosaic_block_size.unwrap_or(8) as f32;

    let engine = Arc::new(Mutex::new(Engine::new(
        save_path, format, quality, color, size, mosaic,
    )));
    let engine_clone = Arc::clone(&engine);

    thread::spawn(move || {
        #[cfg(target_os = "macos")]
        {
            use crate::platform::macos::capture_sck::MacOsSckCapture;
            let capture = MacOsSckCapture::new();
            engine_clone.lock().unwrap().start(&capture);
            // Overlay run would go here in future tasks
        }
    });

    Ok(ScreenshotSession { engine })
}

fn parse_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Color::new(r, g, b, 255);
        }
    }
    Color::new(255, 0, 0, 255)
}
