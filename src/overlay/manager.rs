use crate::core::capture::ScreenFrame;
use crate::core::engine::Engine;
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
        }
    }
}

impl ApplicationHandler for OverlayApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("Screenshot Overlay")
            .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false);
        let window = event_loop.create_window(window_attributes).unwrap();

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
        let app = ScreenshotApp::new(Arc::clone(&self.engine));
        let painter = egui_glow::Painter::new(gl.gl.clone(), "", None, true)
            .expect("Failed to create egui_glow Painter");

        self.window = Some(window);
        self.gl_context = Some(gl);
        self.egui_ctx = Some(egui_ctx);
        self.egui_state = Some(egui_state);
        self.screenshot_app = Some(app);
        self.painter = Some(painter);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else { return };
        let Some(gl) = self.gl_context.as_ref() else { return };
        let Some(egui_ctx) = self.egui_ctx.as_ref() else { return };
        let Some(egui_state) = self.egui_state.as_mut() else { return };
        let Some(app) = self.screenshot_app.as_mut() else { return };
        let Some(painter) = self.painter.as_mut() else { return };

        let response = egui_state.on_window_event(window, &event);
        if response.consumed {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
                {
                    self.engine.lock().unwrap().cancel();
                    event_loop.exit();
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
                    app.update(ctx, &mut egui::Frame::none(), self.last_frame_time_ms, self.last_egui_paint_ms);
                });
                egui_state.handle_platform_output(window, full_output.platform_output);

                let clipped_primitives = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
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
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
