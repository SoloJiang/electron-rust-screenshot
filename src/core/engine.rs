use super::capture::{CaptureError, PlatformCapture, ScreenFrame};
use super::editor::EditorState;
use super::events::{EngineEvent, ErrorCode, EventBus};
use super::perf::PerformanceMonitor;
use super::types::{Color, LogicalPoint, Rect, ScreenInfo};
use super::window::WindowDetector;
use crate::overlay::save::composite_and_save;
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
    pub show_debug_hud: bool,
    pub last_metrics: Option<super::events::MetricsPayload>,
    pub frames: Vec<ScreenFrame>,
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
            show_debug_hud: false,
            last_metrics: None,
            frames: Vec::new(),
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
                self.frames = frames;
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

    pub fn save(&mut self, frames: &[ScreenFrame]) {
        self.state = EngineState::Saving;
        match composite_and_save(frames, &self.editor, &self.save_path, &self.format, self.quality) {
            Ok(path) => {
                self.event_bus.emit(EngineEvent::Saved {
                    path,
                    copied: false,
                });
            }
            Err(msg) => {
                self.event_bus.emit(EngineEvent::Error {
                    code: ErrorCode::SaveFailed,
                    message: msg,
                });
            }
        }
        self.state = EngineState::Idle;
    }

    pub fn on_mouse_move(&mut self, pos: LogicalPoint) {
        if let Some(detector) = &self.detector {
            let hit = detector.hit_test(pos).cloned();
            if let Some(win) = hit {
                self.event_bus.emit(EngineEvent::WindowHovered { window: win });
            }
        }
    }

    pub fn on_mouse_down(&mut self, pos: LogicalPoint) {
        if matches!(self.state, EngineState::OverlayRunning) {
            self.state = EngineState::FreeSelecting { start: pos, current: pos };
        }
    }

    pub fn on_mouse_drag(&mut self, pos: LogicalPoint) {
        if let EngineState::FreeSelecting { start, .. } = &self.state {
            if start.distance_sq(pos) > 16.0 {
                if let EngineState::FreeSelecting { ref mut current, .. } = self.state {
                    *current = pos;
                }
            }
        }
    }

    pub fn on_mouse_up(&mut self, screen_id: String, _pos: LogicalPoint) {
        if let EngineState::FreeSelecting { start, current } = self.state {
            let dx = (current.x - start.x).abs();
            let dy = (current.y - start.y).abs();
            if dx < 4.0 && dy < 4.0 && dx * dy < 16.0 {
                if let Some(detector) = &self.detector {
                    if let Some(win) = detector.hit_test(start) {
                        let rect = win.bounds;
                        self.select_region(screen_id, rect);
                        return;
                    }
                }
                self.state = EngineState::OverlayRunning;
            } else {
                let rect = Rect::new(
                    start.x.min(current.x),
                    start.y.min(current.y),
                    dx,
                    dy,
                );
                self.select_region(screen_id, rect);
            }
        }
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
