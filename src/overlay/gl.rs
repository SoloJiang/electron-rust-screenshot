use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextAttributesBuilder, NotCurrentGlContext};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{SurfaceAttributesBuilder, WindowSurface};
use glutin_winit::GlWindow;
use raw_window_handle::HasWindowHandle;
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::window::Window;

pub struct GlContext {
    pub gl_context: glutin::context::PossiblyCurrentContext,
    pub gl_surface: glutin::surface::Surface<WindowSurface>,
    pub gl: Arc<glow::Context>,
}

impl GlContext {
    /// # Safety
    /// Must be called on the main thread with a valid active window and event loop.
    pub unsafe fn new(window: &Window, event_loop: &winit::event_loop::ActiveEventLoop) -> Self {
        let window_handle = window.window_handle().unwrap();

        let gl_config = glutin_winit::DisplayBuilder::new()
            .with_preference(glutin_winit::ApiPreference::FallbackEgl)
            .build(event_loop, ConfigTemplateBuilder::new(), |configs| {
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() > accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .unwrap()
            })
            .unwrap()
            .1;

        let gl_display = gl_config.display();

        let surface_attrs = window
            .build_surface_attributes(SurfaceAttributesBuilder::<WindowSurface>::default())
            .unwrap();
        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&gl_config, &surface_attrs)
                .unwrap()
        };

        let context_attributes = ContextAttributesBuilder::new().build(Some(window_handle.into()));
        let gl_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .unwrap()
                .make_current(&gl_surface)
                .unwrap()
        };

        let gl = Arc::new(glow::Context::from_loader_function(|s| {
            gl_display.get_proc_address(&std::ffi::CString::new(s).unwrap())
        }));

        Self {
            gl_context,
            gl_surface,
            gl,
        }
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.gl_surface.resize(
            &self.gl_context,
            NonZeroU32::new(width.max(1)).unwrap(),
            NonZeroU32::new(height.max(1)).unwrap(),
        );
    }

    pub fn swap_buffers(&self) {
        self.gl_surface.swap_buffers(&self.gl_context).unwrap();
    }
}
