use crate::core::capture::ScreenFrame;
use crate::core::engine::Engine;
use crate::core::types::{LogicalPoint, Rect};
use crate::overlay::app::ScreenshotApp;
use crate::overlay::gl::GlContext;
use egui_winit::State as EguiState;
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
        let mut app = OverlayApp::new(self.engine, self.frames);
        let _ = event_loop.run_app(&mut app);
    }
}

struct OverlayApp {
    engine: Arc<Mutex<Engine>>,
    _frames: Vec<ScreenFrame>,
    window: Option<Window>,
    gl_context: Option<GlContext>,
    egui_ctx: Option<egui::Context>,
    egui_state: Option<EguiState>,
    screenshot_app: Option<ScreenshotApp>,
    painter: Option<egui_glow::Painter>,
    last_frame_time_ms: f64,
    last_egui_paint_ms: f64,
    start_time: Option<std::time::Instant>,
    mock_drag_done: bool,
    focus_attempts: u32,
    modifiers: winit::keyboard::ModifiersState,
    window_offset: LogicalPoint,
}

impl OverlayApp {
    fn new(engine: Arc<Mutex<Engine>>, frames: Vec<ScreenFrame>) -> Self {
        Self {
            engine,
            _frames: frames,
            window: None,
            gl_context: None,
            egui_ctx: None,
            egui_state: None,
            screenshot_app: None,
            painter: None,
            last_frame_time_ms: 0.0,
            last_egui_paint_ms: 0.0,
            start_time: None,
            mock_drag_done: false,
            focus_attempts: 0,
            modifiers: winit::keyboard::ModifiersState::empty(),
            window_offset: LogicalPoint::new(0.0, 0.0),
        }
    }
}

impl ApplicationHandler for OverlayApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let union_rect = self._frames.iter().fold(None, |acc: Option<Rect>, f| {
            let b = f.logical_bounds;
            Some(match acc {
                None => Rect::new(b.x, b.y, b.w, b.h),
                Some(r) => {
                    let min_x = r.x.min(b.x);
                    let min_y = r.y.min(b.y);
                    let max_x = (r.x + r.w).max(b.x + b.w);
                    let max_y = (r.y + r.h).max(b.y + b.h);
                    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
                }
            })
        });
        let rect = union_rect.unwrap_or(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let window_attributes = Window::default_attributes()
            .with_title("Screenshot Overlay")
            .with_inner_size(winit::dpi::LogicalSize::new(rect.w, rect.h))
            .with_position(winit::dpi::LogicalPosition::new(rect.x, rect.y))
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
        let egui_ctx = egui::Context::default();
        let egui_state = EguiState::new(
            egui_ctx.clone(),
            egui::ViewportId::default(),
            &window,
            Some(window.scale_factor() as f32),
            None,
            None::<usize>,
        );
        let frames = std::mem::take(&mut self._frames);
        let offset = LogicalPoint::new(rect.x, rect.y);
        let mut app = ScreenshotApp::new(Arc::clone(&self.engine), frames);
        app.load_screenshot_textures(&egui_ctx);
        let painter = egui_glow::Painter::new(gl.gl.clone(), "", None, true)
            .expect("Failed to create egui_glow Painter");

        self.window_offset = offset;
        self.window = Some(window);
        self.gl_context = Some(gl);
        self.egui_ctx = Some(egui_ctx);
        self.egui_state = Some(egui_state);
        self.screenshot_app = Some(app);
        self.painter = Some(painter);
        self.start_time = Some(std::time::Instant::now());
        if let Some(window) = &self.window {
            window.focus_window();
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let Some(gl) = self.gl_context.as_ref() else {
            return;
        };
        let Some(egui_ctx) = self.egui_ctx.as_ref() else {
            return;
        };
        let Some(egui_state) = self.egui_state.as_mut() else {
            return;
        };
        let Some(app) = self.screenshot_app.as_mut() else {
            return;
        };
        let Some(painter) = self.painter.as_mut() else {
            return;
        };

        let response = egui_state.on_window_event(window, &event);
        if response.consumed {
            return;
        }

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
                let size = window.inner_size();
                gl.resize(size.width, size.height);
                unsafe {
                    use glow::HasContext;
                    gl.gl.viewport(0, 0, size.width as i32, size.height as i32);
                    gl.gl.clear_color(0.0, 0.0, 0.0, 0.0);
                    gl.gl.clear(glow::COLOR_BUFFER_BIT);
                }

                let raw_input = egui_state.take_egui_input(window);
                let full_output = egui_ctx.run(raw_input, |ctx| {
                    app.update(
                        ctx,
                        &mut egui::Frame::none(),
                        self.window_offset,
                        self.last_frame_time_ms,
                        self.last_egui_paint_ms,
                    );
                });
                egui_state.handle_platform_output(window, full_output.platform_output);

                let clipped_primitives =
                    egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
                let ppp = egui_ctx.native_pixels_per_point().unwrap_or(1.0);
                let paint_start = std::time::Instant::now();
                painter.paint_and_update_textures(
                    [size.width, size.height],
                    ppp,
                    &clipped_primitives,
                    &full_output.textures_delta,
                );
                self.last_egui_paint_ms = paint_start.elapsed().as_secs_f64() * 1000.0;

                gl.swap_buffers();
                self.last_frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
                window.request_redraw();
            }
            _ => {}
        }

        if self.engine.lock().unwrap().should_close {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
            if self.focus_attempts < 60 {
                window.focus_window();
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
                    if let Ok(handle) = window.window_handle() {
                        if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                            let ns_view: *mut Object = appkit.ns_view.as_ptr() as *mut Object;
                            let ns_window: *mut Object = msg_send![ns_view, window];
                            if !ns_window.is_null() {
                                let _: () = msg_send![ns_window, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
                            }
                        }
                    }
                }
                self.focus_attempts += 1;
            }
        }
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
