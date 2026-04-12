use crate::core::capture::ScreenFrame;
use crate::core::engine::Engine;
use crate::core::types::{LogicalPoint, Rect};
use crate::overlay::app::ScreenshotApp;
use crate::overlay::gl::GlContext;
use egui_winit::State as EguiState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

pub struct OverlayManager {
    engine: Arc<Mutex<Engine>>,
    frames: Vec<ScreenFrame>,
}

impl OverlayManager {
    pub fn new(engine: Arc<Mutex<Engine>>, frames: Vec<ScreenFrame>) -> Self {
        Self { engine, frames }
    }

    pub fn run(self) {
        let event_loop = EventLoop::new().expect("Failed to create event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = MultiWindowApp::new(self.engine);
        app.pending_frames = self.frames;
        let _ = event_loop.run_app(&mut app);
    }
}

struct WindowState {
    window: Window,
    gl_context: GlContext,
    egui_state: EguiState,
    painter: egui_glow::Painter,
    screen_id: String,
    global_offset: LogicalPoint,
    screen_bounds: Rect,
    ignores_mouse_events: bool,
    last_frame_time_ms: f64,
    last_egui_paint_ms: f64,
}

struct MultiWindowApp {
    engine: Arc<Mutex<Engine>>,
    egui_ctx: egui::Context,
    screenshot_app: Option<ScreenshotApp>,
    frame_textures: Vec<Option<egui::TextureHandle>>,
    windows: HashMap<winit::window::WindowId, WindowState>,
    start_time: Option<std::time::Instant>,
    mock_drag_done: bool,
    focus_attempts: u32,
    modifiers: winit::keyboard::ModifiersState,
    pending_frames: Vec<ScreenFrame>,
}

impl MultiWindowApp {
    fn new(engine: Arc<Mutex<Engine>>) -> Self {
        Self {
            engine,
            egui_ctx: egui::Context::default(),
            screenshot_app: None,
            frame_textures: Vec::new(),
            windows: HashMap::new(),
            start_time: None,
            mock_drag_done: false,
            focus_attempts: 0,
            modifiers: winit::keyboard::ModifiersState::empty(),
            pending_frames: Vec::new(),
        }
    }

    fn update_interactivity(&mut self) {
        let engine = self.engine.lock().unwrap();
        let active_id = match engine.state {
            crate::core::engine::EngineState::Editing => {
                engine.editor.selection.and_then(|sel| {
                    let cx = sel.x + sel.w / 2.0;
                    let cy = sel.y + sel.h / 2.0;
                    self.windows.iter().find(|(_, ws)| {
                        ws.screen_bounds
                            .contains(LogicalPoint::new(cx, cy))
                    }).map(|(id, _)| *id)
                })
            }
            _ => None,
        };
        drop(engine);

        for (id, ws) in self.windows.iter_mut() {
            let should_ignore = active_id.map(|a| a != *id).unwrap_or(false);
            if should_ignore != ws.ignores_mouse_events {
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc::msg_send;
                    use objc::sel;
                    use objc::sel_impl;
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    use objc::runtime::Object;
                    if let Ok(handle) = ws.window.window_handle() {
                        if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                            let ns_view: *mut Object = appkit.ns_view.as_ptr() as *mut Object;
                            let ns_window: *mut Object = msg_send![ns_view, window];
                            if !ns_window.is_null() {
                                let _: () = msg_send![ns_window, setIgnoresMouseEvents: should_ignore];
                            }
                        }
                    }
                }
                ws.ignores_mouse_events = should_ignore;
            }
        }
    }
}

impl ApplicationHandler for MultiWindowApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let frames = std::mem::take(&mut self.pending_frames);
        if frames.is_empty() {
            return;
        }
        let mut screenshot_app = ScreenshotApp::new(Arc::clone(&self.engine), frames.clone());
        screenshot_app.load_screenshot_textures(&self.egui_ctx);
        self.frame_textures = screenshot_app.frame_textures.clone();
        self.screenshot_app = Some(screenshot_app);

        for frame in frames {
            let b = frame.logical_bounds;
            let window_attributes = Window::default_attributes()
                .with_title("Screenshot Overlay")
                .with_inner_size(winit::dpi::LogicalSize::new(b.w, b.h))
                .with_position(winit::dpi::LogicalPosition::new(b.x, b.y))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false);

            let window = event_loop.create_window(window_attributes).unwrap();

            #[cfg(target_os = "macos")]
            unsafe {
                use objc::class;
                use objc::msg_send;
                use objc::runtime::Object;
                use objc::sel;
                use objc::sel_impl;
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
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

            let gl = unsafe { GlContext::new(&window, event_loop) };
            let egui_state = EguiState::new(
                self.egui_ctx.clone(),
                egui::ViewportId::default(),
                &window,
                Some(window.scale_factor() as f32),
                None,
                None::<usize>,
            );
            let painter = egui_glow::Painter::new(gl.gl.clone(), "", None, true)
                .expect("Failed to create egui_glow Painter");

            let ws = WindowState {
                window,
                gl_context: gl,
                egui_state,
                painter,
                screen_id: frame.screen_id.clone(),
                global_offset: LogicalPoint::new(b.x, b.y),
                screen_bounds: b,
                ignores_mouse_events: false,
                last_frame_time_ms: 0.0,
                last_egui_paint_ms: 0.0,
            };
            let id = ws.window.id();
            self.windows.insert(id, ws);
        }

        self.start_time = Some(std::time::Instant::now());
        for ws in self.windows.values() {
            ws.window.focus_window();
            ws.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(app) = self.screenshot_app.as_mut() else {
            return;
        };

        let response = {
            let Some(ws) = self.windows.get_mut(&window_id) else {
                return;
            };
            ws.egui_state.on_window_event(&ws.window, &event)
        };

        if response.consumed {
            return;
        }

        let Some(ws) = self.windows.get_mut(&window_id) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == winit::event::ElementState::Pressed =>
            {
                let is_cmd = self.modifiers.super_key();
                let is_shift = self.modifiers.shift_key();

                if event.logical_key
                    == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
                {
                    self.engine.lock().unwrap().cancel();
                    event_loop.exit();
                }
                if event.logical_key
                    == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter)
                {
                    let frames = &app.frames;
                    self.engine.lock().unwrap().save(frames);
                }

                // Undo / Redo
                if is_cmd && !is_shift {
                    if let winit::keyboard::Key::Character(c) = &event.logical_key {
                        if c.eq_ignore_ascii_case("z") {
                            self.engine.lock().unwrap().editor.undo();
                        }
                    }
                }
                if is_cmd && is_shift {
                    if let winit::keyboard::Key::Character(c) = &event.logical_key {
                        if c.eq_ignore_ascii_case("z") {
                            self.engine.lock().unwrap().editor.redo();
                        }
                    }
                }

                // Copy to clipboard and tool switching (only in Editing state)
                {
                    let mut engine = self.engine.lock().unwrap();
                    if matches!(engine.state, crate::core::engine::EngineState::Editing) {
                        if let winit::keyboard::Key::Character(c) = &event.logical_key {
                            let key = c.as_str();
                            match key {
                                "c" | "C" => {
                                    let frames = &app.frames;
                                    engine.copy_to_clipboard(frames);
                                }
                                "1" => engine.editor.active_tool = crate::core::editor::Tool::Rect,
                                "2" => {
                                    engine.editor.active_tool = crate::core::editor::Tool::Ellipse
                                }
                                "3" => engine.editor.active_tool = crate::core::editor::Tool::Arrow,
                                "4" => engine.editor.active_tool = crate::core::editor::Tool::Brush,
                                "5" => {
                                    engine.editor.active_tool = crate::core::editor::Tool::Mosaic
                                }
                                "6" => engine.editor.active_tool = crate::core::editor::Tool::Text,
                                _ => {}
                            }
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let frame_start = std::time::Instant::now();
                let size = ws.window.inner_size();
                ws.gl_context.resize(size.width, size.height);
                unsafe {
                    use glow::HasContext;
                    ws.gl_context.gl.viewport(0, 0, size.width as i32, size.height as i32);
                    ws.gl_context.gl.clear_color(0.0, 0.0, 0.0, 0.0);
                    ws.gl_context.gl.clear(glow::COLOR_BUFFER_BIT);
                }

                let raw_input = ws.egui_state.take_egui_input(&ws.window);
                let offset = ws.global_offset;
                let full_output = self.egui_ctx.run(raw_input, |ctx| {
                    app.update(
                        ctx,
                        &mut egui::Frame::none(),
                        offset,
                        ws.last_frame_time_ms,
                        ws.last_egui_paint_ms,
                    );
                });
                ws.egui_state.handle_platform_output(&ws.window, full_output.platform_output);

                let clipped_primitives =
                    self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
                let ppp = self.egui_ctx.native_pixels_per_point().unwrap_or(1.0);
                let paint_start = std::time::Instant::now();
                ws.painter.paint_and_update_textures(
                    [size.width, size.height],
                    ppp,
                    &clipped_primitives,
                    &full_output.textures_delta,
                );
                ws.last_egui_paint_ms = paint_start.elapsed().as_secs_f64() * 1000.0;

                ws.gl_context.swap_buffers();
                ws.last_frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
                ws.window.request_redraw();
            }
            _ => {}
        }

        self.update_interactivity();

        if self.engine.lock().unwrap().should_close {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        for ws in self.windows.values() {
            ws.window.request_redraw();
            if self.focus_attempts < 60 {
                ws.window.focus_window();
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc::class;
                    use objc::msg_send;
                    use objc::runtime::Object;
                    use objc::sel;
                    use objc::sel_impl;
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                    let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
                    if let Ok(handle) = ws.window.window_handle() {
                        if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                            let ns_view: *mut Object = appkit.ns_view.as_ptr() as *mut Object;
                            let ns_window: *mut Object = msg_send![ns_view, window];
                            if !ns_window.is_null() {
                                let _: () = msg_send![ns_window, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
                            }
                        }
                    }
                }
            }
        }
        if self.focus_attempts < 60 {
            self.focus_attempts += 1;
        }

        self.update_interactivity();

        if self.engine.lock().unwrap().should_close {
            event_loop.exit();
            return;
        }

        if let Some(start) = self.start_time {
            if !self.mock_drag_done {
                if let Ok(mock_drag) = std::env::var("SCREENSHOT_TEST_MOCK_DRAG") {
                    if start.elapsed().as_secs_f64() > 5.0 {
                        let parts: Vec<f64> = mock_drag
                            .split(',')
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        if parts.len() == 4 {
                            let start_pt = LogicalPoint::new(parts[0], parts[1]);
                            let end_pt = LogicalPoint::new(parts[2], parts[3]);
                            let mut engine = self.engine.lock().unwrap();
                            engine.on_mouse_down(start_pt);
                            engine.on_mouse_drag(end_pt);
                            engine.on_mouse_up(end_pt);
                            if std::env::var("SCREENSHOT_TEST_MOCK_DRAG_NO_SAVE").is_err() {
                                if let Some(app) = &self.screenshot_app {
                                    engine.save(&app.frames);
                                }
                            }
                            self.mock_drag_done = true;
                        }
                    }
                }
            }
            if let Ok(timeout_str) = std::env::var("SCREENSHOT_TEST_TIMEOUT_MS") {
                if let Ok(timeout_ms) = timeout_str.parse::<u64>() {
                    if start.elapsed().as_millis() as u64 > timeout_ms {
                        self.engine.lock().unwrap().cancel();
                        event_loop.exit();
                    }
                }
            }
        }
    }
}
