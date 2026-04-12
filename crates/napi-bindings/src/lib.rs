#![deny(clippy::all)]

pub mod bridge;

use crate::bridge::config::ScreenshotConfig;
use crate::bridge::events_js::serialize_event;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use screenshot_core::{
    core::engine::Engine,
    core::events::EngineEvent,
    core::types::Color,
    core::window::WindowDetector,
    overlay::manager::OverlayManager,
};
use std::sync::{Arc, Mutex};

#[napi]
pub fn start(config: Option<ScreenshotConfig>) -> Result<String> {
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

    #[cfg(target_os = "macos")]
    {
        use screenshot_core::platform::macos::capture_sck::MacOsSckCapture;
        let capture = MacOsSckCapture::new();
        {
            let mut eng = engine.lock().unwrap();
            eng.start(&capture);
            if matches!(eng.state, screenshot_core::core::engine::EngineState::Idle) {
                // Capture failed, drain events and return error
                let mut events = Vec::new();
                while let Some(evt) = eng.event_bus.try_recv() {
                    events.push(evt);
                }
                if let Some(EngineEvent::Error { message, .. }) = events.into_iter().next_back() {
                    return Err(Error::from_reason(message));
                }
                return Err(Error::from_reason("Capture failed".to_string()));
            }
            let windows = screenshot_core::platform::macos::window::enumerate_windows();
            eng.detector = Some(WindowDetector::new(windows));
        }
        let frames = engine.lock().unwrap().frames.clone();
        OverlayManager::new(Arc::clone(&engine), frames).run();
    }

    // Overlay closed, collect final event
    let mut events = Vec::new();
    {
        let eng = engine.lock().unwrap();
        while let Some(evt) = eng.event_bus.try_recv() {
            events.push(evt);
        }
    }
    let final_event = events.into_iter().rev().find(|e| {
        matches!(
            e,
            EngineEvent::Saved { .. } | EngineEvent::Cancelled | EngineEvent::Error { .. }
        )
    });
    if let Some(evt) = final_event {
        Ok(serialize_event(&evt))
    } else {
        Ok(r#"{"type":"cancelled"}"#.to_string())
    }
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
