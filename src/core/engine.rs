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
    FreeSelecting {
        start: LogicalPoint,
        current: LogicalPoint,
    },
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
    pub edit_drag_start: Option<LogicalPoint>,
    pub hovered_window: Option<super::types::DetectedWindow>,
    pub should_close: bool,
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
            edit_drag_start: None,
            hovered_window: None,
            should_close: false,
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
        self.should_close = true;
        self.event_bus.emit(EngineEvent::Cancelled);
    }

    pub fn select_region(&mut self, screen_id: String, rect: Rect) {
        if rect.w > 8.0 && rect.h > 8.0 {
            self.editor.selection = Some(rect);
            self.state = EngineState::Editing;
            self.event_bus
                .emit(EngineEvent::RegionSelected { screen_id, rect });
        } else {
            self.state = EngineState::OverlayRunning;
        }
    }

    pub fn save(&mut self, frames: &[ScreenFrame]) {
        self.state = EngineState::Saving;
        match composite_and_save(
            frames,
            &self.editor,
            &self.save_path,
            &self.format,
            self.quality,
        ) {
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
        self.should_close = true;
    }

    pub fn copy_to_clipboard(&mut self, frames: &[ScreenFrame]) {
        #[cfg(target_os = "macos")]
        {
            match crate::overlay::save::composite_image(frames, &self.editor) {
                Ok(img) => match crate::platform::macos::clipboard::copy_image_to_clipboard(&img) {
                    Ok(()) => {
                        self.event_bus.emit(EngineEvent::Saved {
                            path: "clipboard".into(),
                            copied: true,
                        });
                    }
                    Err(msg) => {
                        self.event_bus.emit(EngineEvent::Error {
                            code: ErrorCode::SaveFailed,
                            message: msg,
                        });
                    }
                },
                Err(msg) => {
                    self.event_bus.emit(EngineEvent::Error {
                        code: ErrorCode::SaveFailed,
                        message: msg,
                    });
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = frames;
            self.event_bus.emit(EngineEvent::Error {
                code: ErrorCode::SaveFailed,
                message: "Clipboard not supported on this platform".into(),
            });
        }
    }

    pub fn on_mouse_move(&mut self, pos: LogicalPoint) {
        if let Some(detector) = &self.detector {
            let hit = detector.hit_test(pos).cloned();
            if hit != self.hovered_window {
                self.hovered_window = hit.clone();
                if let Some(win) = hit {
                    self.event_bus
                        .emit(EngineEvent::WindowHovered { window: win });
                }
            }
        }
    }

    pub fn on_edit_mouse_down(&mut self, pos: LogicalPoint) {
        if matches!(self.state, EngineState::Editing) {
            self.edit_drag_start = Some(pos);
            self.editor.clear_preview();
        }
    }

    pub fn on_edit_mouse_drag(&mut self, pos: LogicalPoint) {
        if let (EngineState::Editing, Some(start)) = (&self.state, self.edit_drag_start) {
            self.editor.preview = build_preview(
                &self.editor.active_tool,
                start,
                pos,
                self.editor.tool_color,
                self.editor.tool_size,
                self.editor.mosaic_block_size,
            );
        }
    }

    pub fn on_edit_mouse_up(&mut self, pos: LogicalPoint) {
        if let (EngineState::Editing, Some(start)) = (&self.state, self.edit_drag_start) {
            if let Some(preview) = self.editor.preview.take() {
                self.editor.add_layer(preview);
            } else {
                // Single click with minimal movement: create minimal shape
                if let Some(layer) = build_preview(
                    &self.editor.active_tool,
                    start,
                    pos,
                    self.editor.tool_color,
                    self.editor.tool_size,
                    self.editor.mosaic_block_size,
                ) {
                    self.editor.add_layer(layer);
                }
            }
            self.edit_drag_start = None;
        }
    }

    pub fn on_mouse_down(&mut self, pos: LogicalPoint) {
        if matches!(self.state, EngineState::OverlayRunning) {
            self.state = EngineState::FreeSelecting {
                start: pos,
                current: pos,
            };
        }
    }

    pub fn on_mouse_drag(&mut self, pos: LogicalPoint) {
        if let EngineState::FreeSelecting { ref mut current, .. } = self.state {
            *current = pos;
        }
    }

    pub fn screen_at_point(&self, pos: LogicalPoint) -> Option<String> {
        self.screens
            .iter()
            .find(|s| s.logical_bounds.contains(pos))
            .map(|s| s.id.clone())
    }

    pub fn on_mouse_up(&mut self, pos: LogicalPoint) {
        let screen_id = self
            .screen_at_point(pos)
            .unwrap_or_else(|| "primary".to_string());
        if let EngineState::FreeSelecting { start, .. } = self.state {
            let dx = (pos.x - start.x).abs();
            let dy = (pos.y - start.y).abs();
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
                let rect = Rect::new(start.x.min(pos.x), start.y.min(pos.y), dx, dy);
                self.select_region(screen_id, rect);
            }
        }
    }
}

fn build_preview(
    tool: &super::editor::Tool,
    start: LogicalPoint,
    end: LogicalPoint,
    color: Color,
    size: f32,
    mosaic_block_size: f32,
) -> Option<super::editor::Layer> {
    use super::editor::Layer;
    use uuid::Uuid;
    let id = Uuid::new_v4().to_string();
    match tool {
        super::editor::Tool::Rect => Some(Layer::ShapeRect {
            id,
            rect: Rect::new(
                start.x.min(end.x),
                start.y.min(end.y),
                (end.x - start.x).abs(),
                (end.y - start.y).abs(),
            ),
            stroke_width: size,
            color,
        }),
        super::editor::Tool::Ellipse => Some(Layer::ShapeEllipse {
            id,
            rect: Rect::new(
                start.x.min(end.x),
                start.y.min(end.y),
                (end.x - start.x).abs(),
                (end.y - start.y).abs(),
            ),
            stroke_width: size,
            color,
        }),
        super::editor::Tool::Arrow => Some(Layer::Arrow {
            id,
            start,
            end,
            stroke_width: size,
            color,
        }),
        super::editor::Tool::Brush => Some(Layer::BrushPath {
            id,
            points: vec![start, end],
            stroke_width: size,
            color,
        }),
        super::editor::Tool::Mosaic => Some(Layer::MosaicPath {
            id,
            points: vec![start, end],
            block_size: mosaic_block_size,
        }),
        super::editor::Tool::Text => None, // handled later via text input
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
    use crate::core::types::DetectedWindow;
    use crate::core::window::WindowDetector;

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

    #[test]
    fn engine_edit_drag_creates_rect_layer() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::Editing;
        engine.editor.selection = Some(Rect::new(0.0, 0.0, 500.0, 500.0));
        engine.editor.active_tool = crate::core::editor::Tool::Rect;

        engine.on_edit_mouse_down(LogicalPoint::new(10.0, 10.0));
        engine.on_edit_mouse_drag(LogicalPoint::new(100.0, 100.0));
        assert!(engine.editor.preview.is_some());

        engine.on_edit_mouse_up(LogicalPoint::new(100.0, 100.0));
        assert_eq!(engine.editor.layers.len(), 1);
        assert!(engine.editor.preview.is_none());
        assert!(matches!(
            engine.editor.layers[0],
            crate::core::editor::Layer::ShapeRect { .. }
        ));
    }

    #[test]
    fn engine_window_hover_emits_event_on_change() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        let window = DetectedWindow {
            id: "w1".into(),
            title: "Test".into(),
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            owner_pid: 1,
            z_order: 1,
        };
        engine.detector = Some(WindowDetector::new(vec![window.clone()]));

        engine.on_mouse_move(LogicalPoint::new(50.0, 50.0));
        let event = engine.event_bus.try_recv();
        assert!(
            matches!(event, Some(EngineEvent::WindowHovered { ref window }) if window.id == "w1")
        );

        // Moving again inside the same window should not emit another event
        engine.on_mouse_move(LogicalPoint::new(60.0, 60.0));
        assert!(engine.event_bus.try_recv().is_none());
    }

    #[test]
    fn engine_copy_without_selection_emits_error() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.copy_to_clipboard(&[]);
        let event = engine.event_bus.try_recv();
        assert!(matches!(
            event,
            Some(EngineEvent::Error {
                code: ErrorCode::SaveFailed,
                ..
            })
        ));
    }

    #[test]
    fn engine_undo_redo_restores_layers() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::Editing;
        engine.editor.selection = Some(Rect::new(0.0, 0.0, 500.0, 500.0));
        engine.editor.active_tool = crate::core::editor::Tool::Rect;

        engine.on_edit_mouse_down(LogicalPoint::new(10.0, 10.0));
        engine.on_edit_mouse_up(LogicalPoint::new(100.0, 100.0));
        assert_eq!(engine.editor.layers.len(), 1);

        engine.editor.undo();
        assert!(engine.editor.layers.is_empty());

        engine.editor.redo();
        assert_eq!(engine.editor.layers.len(), 1);
    }

    #[test]
    fn engine_screen_at_point_finds_correct_screen() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.screens = vec![
            ScreenInfo {
                id: "left".into(),
                name: "Left".into(),
                logical_bounds: Rect::new(0.0, 0.0, 1000.0, 500.0),
                dpi_scale: 2.0,
            },
            ScreenInfo {
                id: "right".into(),
                name: "Right".into(),
                logical_bounds: Rect::new(1000.0, 0.0, 1000.0, 500.0),
                dpi_scale: 2.0,
            },
        ];
        assert_eq!(engine.screen_at_point(LogicalPoint::new(100.0, 100.0)), Some("left".into()));
        assert_eq!(engine.screen_at_point(LogicalPoint::new(1100.0, 100.0)), Some("right".into()));
        assert_eq!(engine.screen_at_point(LogicalPoint::new(9999.0, 9999.0)), None);
    }

    #[test]
    fn engine_mouse_up_auto_selects_screen() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.screens = vec![
            ScreenInfo {
                id: "left".into(),
                name: "Left".into(),
                logical_bounds: Rect::new(0.0, 0.0, 1000.0, 500.0),
                dpi_scale: 1.0,
            },
        ];
        engine.state = EngineState::OverlayRunning;
        // trigger free-select
        engine.on_mouse_down(LogicalPoint::new(10.0, 10.0));
        engine.on_mouse_drag(LogicalPoint::new(100.0, 100.0));
        engine.on_mouse_up(LogicalPoint::new(100.0, 100.0));
        assert!(matches!(engine.state, EngineState::Editing));
    }
}
