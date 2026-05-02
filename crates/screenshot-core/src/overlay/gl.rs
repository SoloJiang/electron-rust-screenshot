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
    pub unsafe fn new(
        window: &Window,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) -> Result<Self, String> {
        let window_handle = window
            .window_handle()
            .map_err(|e| format!("window_handle failed: {e}"))?;

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
                    .expect("No GL config found")
            })
            .map_err(|e| format!("DisplayBuilder failed: {e}"))?
            .1;

        let gl_display = gl_config.display();

        let surface_attrs = window
            .build_surface_attributes(SurfaceAttributesBuilder::<WindowSurface>::default())
            .map_err(|e| format!("build_surface_attributes failed: {e}"))?;
        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&gl_config, &surface_attrs)
                .map_err(|e| format!("create_window_surface failed: {e}"))?
        };

        let context_attributes = ContextAttributesBuilder::new().build(Some(window_handle.into()));
        let gl_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .map_err(|e| format!("create_context failed: {e}"))?
                .make_current(&gl_surface)
                .map_err(|e| format!("make_current failed: {e}"))?
        };

        let mut gl_load_err: Option<String> = None;
        let gl = Arc::new(glow::Context::from_loader_function(
            |s| match std::ffi::CString::new(s) {
                Ok(c_str) => gl_display.get_proc_address(&c_str),
                Err(e) => {
                    gl_load_err = Some(format!("Invalid GL function string: {e}"));
                    std::ptr::null()
                }
            },
        ));
        if let Some(err) = gl_load_err {
            return Err(err);
        }

        Ok(Self {
            gl_context,
            gl_surface,
            gl,
        })
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.gl_surface.resize(
            &self.gl_context,
            // SAFETY: width.max(1) is always >= 1, so the value is non-zero.
            unsafe { NonZeroU32::new_unchecked(width.max(1)) },
            // SAFETY: height.max(1) is always >= 1, so the value is non-zero.
            unsafe { NonZeroU32::new_unchecked(height.max(1)) },
        );
    }

    pub fn swap_buffers(&self) {
        let _ = self.gl_surface.swap_buffers(&self.gl_context);
    }

    pub fn make_current(&self) -> Result<(), glutin::error::Error> {
        self.gl_context.make_current(&self.gl_surface)
    }
}
