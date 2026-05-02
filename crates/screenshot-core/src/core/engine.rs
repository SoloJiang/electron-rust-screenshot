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

pub struct SelectionTransformState {
    pub kind: super::types::ResizeHit,
    pub start_pointer: LogicalPoint,
    pub original_selection: Rect,
    pub original_layers: Vec<super::editor::Layer>,
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
    pub selection_transform: Option<SelectionTransformState>,
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
            selection_transform: None,
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
        self.selection_transform = None;
        self.state = EngineState::Idle;
        self.should_close = true;
        self.event_bus.emit(EngineEvent::Cancelled);
    }

    pub fn select_region(&mut self, screen_id: String, rect: Rect) {
        self.selection_transform = None;
        if rect.w > 8.0 && rect.h > 8.0 {
            self.editor.selection = Some(rect);
            self.state = EngineState::Editing;
            self.event_bus
                .emit(EngineEvent::RegionSelected { screen_id, rect });
        } else {
            self.state = EngineState::OverlayRunning;
        }
    }

    pub fn save(&mut self) {
        self.selection_transform = None;
        self.state = EngineState::Saving;
        match composite_and_save(
            &self.frames,
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

    pub fn copy_to_clipboard(&mut self) {
        use crate::platform::traits::PlatformClipboard;
        match crate::overlay::save::composite_image(&self.frames, &self.editor) {
            Ok(img) => match crate::platform::Backend::copy_image(&img) {
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

    pub fn select_hovered_window(&mut self) {
        if let (EngineState::OverlayRunning, Some(win)) = (&self.state, self.hovered_window.clone())
        {
            let screen_id = self
                .screen_at_point(LogicalPoint::new(win.bounds.x, win.bounds.y))
                .unwrap_or_else(|| "primary".to_string());
            self.select_region(screen_id, win.bounds);
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

    pub fn abort_edit_drag(&mut self) {
        if matches!(self.state, EngineState::Editing) {
            self.edit_drag_start = None;
            self.editor.clear_preview();
        }
    }

    pub fn on_selection_transform_start(
        &mut self,
        pos: LogicalPoint,
        kind: super::types::ResizeHit,
    ) {
        if let EngineState::Editing = self.state {
            if let Some(sel) = self.editor.selection {
                self.selection_transform = Some(SelectionTransformState {
                    kind,
                    start_pointer: pos,
                    original_selection: sel,
                    original_layers: self.editor.layers.clone(),
                });
                self.editor.clear_preview();
            }
        }
    }

    pub fn on_selection_transform_drag(&mut self, pos: LogicalPoint) {
        if let (EngineState::Editing, Some(ref state)) =
            (&self.state, self.selection_transform.as_ref())
        {
            let delta =
                LogicalPoint::new(pos.x - state.start_pointer.x, pos.y - state.start_pointer.y);
            let (new_rect, new_layers) = EditorState::transform_selection(
                state.original_selection,
                &state.original_layers,
                &state.kind,
                delta,
            );
            self.editor.selection = Some(new_rect);
            self.editor.layers = new_layers;
        }
    }

    pub fn on_selection_transform_end(&mut self, _pos: LogicalPoint) {
        if let (EngineState::Editing, Some(state)) = (&self.state, self.selection_transform.take())
        {
            if self.editor.selection != Some(state.original_selection)
                || self.editor.layers != state.original_layers
            {
                self.editor
                    .undo_stack
                    .push(super::editor::LayerOp::UpdateSelectionAndLayers {
                        old_selection: Some(state.original_selection),
                        new_selection: self.editor.selection,
                        old_layers: state.original_layers,
                        new_layers: self.editor.layers.clone(),
                    });
                self.editor.redo_stack.clear();
            }
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
        if let EngineState::FreeSelecting {
            ref mut current, ..
        } = self.state
        {
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

    pub fn snapshot_state(&self) -> harness_protocol::StateSnapshot {
        let state = match self.state {
            EngineState::Idle => "Idle",
            EngineState::Capturing => "Capturing",
            EngineState::OverlayRunning => "OverlayRunning",
            EngineState::FreeSelecting { .. } => "FreeSelecting",
            EngineState::Editing => "Editing",
            EngineState::Saving => "Saving",
        }
        .to_string();

        let active_tool = format!("{:?}", self.editor.active_tool);

        let selection = self
            .editor
            .selection
            .map(|r| serde_json::to_value(r).expect("Rect serializes"));

        let hovered_window = self
            .hovered_window
            .as_ref()
            .map(|w| serde_json::to_value(w).expect("DetectedWindow serializes"));

        let free_selecting = match self.state {
            EngineState::FreeSelecting { start, current } => Some(serde_json::json!({
                "start": { "x": start.x, "y": start.y },
                "current": { "x": current.x, "y": current.y }
            })),
            _ => None,
        };

        harness_protocol::StateSnapshot {
            state,
            selection,
            layers: self.editor.layers.len(),
            active_tool,
            hovered_window,
            free_selecting,
        }
    }

    pub fn set_active_tool(&mut self, tool: crate::core::editor::Tool) {
        self.editor.active_tool = tool;
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
        super::editor::Tool::Select => None,
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
        engine.copy_to_clipboard();
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
        assert_eq!(
            engine.screen_at_point(LogicalPoint::new(100.0, 100.0)),
            Some("left".into())
        );
        assert_eq!(
            engine.screen_at_point(LogicalPoint::new(1100.0, 100.0)),
            Some("right".into())
        );
        assert_eq!(
            engine.screen_at_point(LogicalPoint::new(9999.0, 9999.0)),
            None
        );
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
        engine.screens = vec![ScreenInfo {
            id: "left".into(),
            name: "Left".into(),
            logical_bounds: Rect::new(0.0, 0.0, 1000.0, 500.0),
            dpi_scale: 1.0,
        }];
        engine.state = EngineState::OverlayRunning;
        // trigger free-select
        engine.on_mouse_down(LogicalPoint::new(10.0, 10.0));
        engine.on_mouse_drag(LogicalPoint::new(100.0, 100.0));
        engine.on_mouse_up(LogicalPoint::new(100.0, 100.0));
        assert!(matches!(engine.state, EngineState::Editing));
    }

    #[test]
    fn select_hovered_window_enters_editing() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::OverlayRunning;
        engine.screens = vec![ScreenInfo {
            id: "main".into(),
            name: "Main".into(),
            logical_bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
            dpi_scale: 1.0,
        }];
        let window = DetectedWindow {
            id: "w1".into(),
            title: "Test Window".into(),
            bounds: Rect::new(100.0, 100.0, 400.0, 300.0),
            owner_pid: 42,
            z_order: 1,
        };
        engine.hovered_window = Some(window.clone());

        engine.select_hovered_window();

        assert!(matches!(engine.state, EngineState::Editing));
        assert_eq!(engine.editor.selection, Some(window.bounds));
        let event = engine.event_bus.try_recv();
        assert!(
            matches!(event, Some(EngineEvent::RegionSelected { screen_id, rect }) if screen_id == "main" && rect == window.bounds)
        );
    }

    #[test]
    fn select_hovered_window_noop_without_hover() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::OverlayRunning;
        engine.hovered_window = None;

        engine.select_hovered_window();

        assert!(matches!(engine.state, EngineState::OverlayRunning));
        assert!(engine.editor.selection.is_none());
        assert!(engine.event_bus.try_recv().is_none());
    }

    #[test]
    fn select_hovered_window_noop_in_editing() {
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
        let window = DetectedWindow {
            id: "w1".into(),
            title: "Test Window".into(),
            bounds: Rect::new(100.0, 100.0, 400.0, 300.0),
            owner_pid: 42,
            z_order: 1,
        };
        engine.hovered_window = Some(window);

        engine.select_hovered_window();

        assert!(matches!(engine.state, EngineState::Editing));
        assert_eq!(
            engine.editor.selection,
            Some(Rect::new(0.0, 0.0, 500.0, 500.0))
        );
        assert!(engine.event_bus.try_recv().is_none());
    }

    #[test]
    fn selection_transform_end_creates_undo_record() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::Editing;
        engine.editor.selection = Some(Rect::new(0.0, 0.0, 100.0, 100.0));
        engine.editor.layers = vec![crate::core::editor::Layer::ShapeRect {
            id: "r1".into(),
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        }];

        use crate::core::types::ResizeHit;
        engine.on_selection_transform_start(LogicalPoint::new(0.0, 0.0), ResizeHit::Move);
        engine.on_selection_transform_drag(LogicalPoint::new(50.0, 30.0));
        engine.on_selection_transform_end(LogicalPoint::new(50.0, 30.0));

        assert_eq!(
            engine.editor.selection,
            Some(Rect::new(50.0, 30.0, 100.0, 100.0))
        );
        assert!(!engine.editor.undo_stack.is_empty());
    }

    #[test]
    fn selection_transform_no_move_does_not_create_undo() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::Editing;
        engine.editor.selection = Some(Rect::new(0.0, 0.0, 100.0, 100.0));

        use crate::core::types::ResizeHit;
        engine.on_selection_transform_start(LogicalPoint::new(0.0, 0.0), ResizeHit::Move);
        engine.on_selection_transform_end(LogicalPoint::new(0.0, 0.0));

        assert!(engine.editor.undo_stack.is_empty());
    }

    #[test]
    fn editing_state_allows_tool_switching() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::Editing;
        assert_eq!(engine.editor.active_tool, crate::core::editor::Tool::Select);

        engine.editor.active_tool = crate::core::editor::Tool::Rect;
        assert_eq!(engine.editor.active_tool, crate::core::editor::Tool::Rect);

        engine.editor.active_tool = crate::core::editor::Tool::Brush;
        assert_eq!(engine.editor.active_tool, crate::core::editor::Tool::Brush);

        engine.editor.active_tool = crate::core::editor::Tool::Select;
        assert_eq!(engine.editor.active_tool, crate::core::editor::Tool::Select);
    }

    #[test]
    fn abort_edit_drag_clears_state() {
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
        engine.on_edit_mouse_drag(LogicalPoint::new(50.0, 50.0));
        assert!(engine.edit_drag_start.is_some());
        assert!(engine.editor.preview.is_some());

        engine.abort_edit_drag();
        assert!(engine.edit_drag_start.is_none());
        assert!(engine.editor.preview.is_none());
    }

    #[test]
    fn snapshot_state_reflects_editing_with_layers() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::Editing;
        engine.editor.selection = Some(Rect::new(100.0, 200.0, 300.0, 400.0));
        engine.editor.active_tool = crate::core::editor::Tool::Rect;
        engine
            .editor
            .layers
            .push(crate::core::editor::Layer::ShapeRect {
                id: "x".into(),
                rect: Rect::new(0.0, 0.0, 10.0, 10.0),
                stroke_width: 1.0,
                color: Color::new(0, 0, 0, 255),
            });

        let snap = engine.snapshot_state();
        assert_eq!(snap.state, "Editing");
        assert_eq!(snap.layers, 1);
        assert_eq!(snap.active_tool, "Rect");
        assert!(snap.selection.is_some());
    }
}
