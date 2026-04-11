use super::capture::{CaptureError, PlatformCapture, ScreenFrame};
use super::editor::EditorState;
use super::events::{EngineEvent, ErrorCode, EventBus};
use super::perf::PerformanceMonitor;
use super::types::{Color, LogicalPoint, Rect, ScreenInfo};
use super::window::WindowDetector;
use std::time::Instant;

pub enum EngineState {
    Idle,
    Capturing,
    OverlayRunning,
    FreeSelecting { start: LogicalPoint, current: LogicalPoint },
    Editing,
    Saving,
}

pub struct Engine {
    pub state: EngineState,
    pub event_bus: EventBus,
    pub editor: EditorState,
    pub screens: Vec<ScreenInfo>,
    pub detector: Option<WindowDetector>,
    pub perf: PerformanceMonitor,
    pub save_path: String,
    pub format: String,
    pub quality: u8,
}

impl Engine {
    pub fn new(
        save_path: String,
        format: String,
        quality: u8,
        default_color: Color,
        default_size: f32,
        mosaic_block_size: f32,
    ) -> Self {
        Self {
            state: EngineState::Idle,
            event_bus: EventBus::new(),
            editor: EditorState::new(default_color, default_size, mosaic_block_size),
            screens: Vec::new(),
            detector: None,
            perf: PerformanceMonitor::new(),
            save_path,
            format,
            quality,
        }
    }

    pub fn start(&mut self, capture: &dyn PlatformCapture) {
        self.state = EngineState::Capturing;
        let cap_start = Instant::now();
        match capture.capture_all_screens() {
            Ok(frames) => {
                let _capture_ms = self.perf.record_capture(cap_start);
                self.screens = frames
                    .iter()
                    .map(|f| ScreenInfo {
                        id: f.screen_id.clone(),
                        name: format!("Screen {}", f.screen_id),
                        logical_bounds: f.logical_bounds,
                        dpi_scale: f.dpi_scale,
                    })
                    .collect();
                self.event_bus.emit(EngineEvent::Started {
                    screens: self.screens.clone(),
                });
                self.state = EngineState::OverlayRunning;
            }
            Err(e) => {
                self.event_bus.emit(EngineEvent::Error {
                    code: error_to_code(&e),
                    message: e.to_string(),
                });
                self.state = EngineState::Idle;
            }
        }
    }

    pub fn cancel(&mut self) {
        self.state = EngineState::Idle;
        self.event_bus.emit(EngineEvent::Cancelled);
    }

    pub fn select_region(&mut self, screen_id: String, rect: Rect) {
        if rect.w > 8.0 && rect.h > 8.0 {
            self.editor.selection = Some(rect);
            self.state = EngineState::Editing;
            self.event_bus.emit(EngineEvent::RegionSelected { screen_id, rect });
        } else {
            self.state = EngineState::OverlayRunning;
        }
    }

    pub fn save(&mut self, _frames: &[ScreenFrame]) {
        self.state = EngineState::Saving;
        self.event_bus.emit(EngineEvent::Saved {
            path: self.save_path.clone(),
            copied: false,
        });
        self.state = EngineState::Idle;
    }
}

fn error_to_code(e: &CaptureError) -> ErrorCode {
    match e {
        CaptureError::NoDisplay => ErrorCode::NoDisplay,
        CaptureError::PermissionDenied => ErrorCode::PermissionDenied,
        _ => ErrorCode::CaptureFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::capture::MockCapture;

    #[test]
    fn engine_start_emits_started() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        let cap = MockCapture::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        engine.start(&cap);
        assert!(matches!(engine.state, EngineState::OverlayRunning));
        let event = engine.event_bus.try_recv();
        assert!(matches!(event, Some(EngineEvent::Started { .. })));
    }
}
