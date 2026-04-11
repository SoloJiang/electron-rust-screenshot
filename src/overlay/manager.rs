use crate::core::capture::ScreenFrame;
use crate::core::engine::Engine;
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
    frames: Vec<ScreenFrame>,
    windows: Vec<Window>,
}

impl OverlayApp {
    fn new(engine: Arc<Mutex<Engine>>, frames: Vec<ScreenFrame>) -> Self {
        Self {
            engine,
            frames,
            windows: Vec::new(),
        }
    }
}

impl ApplicationHandler for OverlayApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        for _frame in &self.frames {
            let window_attributes = Window::default_attributes()
                .with_title("Screenshot Overlay")
                .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false);
            let window = event_loop.create_window(window_attributes).unwrap();
            self.windows.push(window);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
                {
                    self.engine.lock().unwrap().cancel();
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}
