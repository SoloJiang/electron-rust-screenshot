use crate::core::engine::Engine;
use crate::core::types::{LogicalPoint, Rect};
use egui::{Color32, Rect as EguiRect, Rounding};
use std::sync::{Arc, Mutex};

pub struct ScreenshotApp {
    pub engine: Arc<Mutex<Engine>>,
    pub frame_texture: Option<egui::TextureHandle>,
}

impl ScreenshotApp {
    pub fn new(engine: Arc<Mutex<Engine>>) -> Self {
        Self {
            engine,
            frame_texture: None,
        }
    }

    pub fn update(&mut self,
        ctx: &egui::Context,
        _frame: &mut egui::Frame,
    ) {
        let mut engine = self.engine.lock().unwrap();

        let panel = egui::CentralPanel::default().frame(egui::Frame::none());
        panel.show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            // 1. Draw background screenshot texture if available
            if let Some(tex) = &self.frame_texture {
                ui.painter().image(
                    tex.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }

            // 2. Handle mouse interaction based on engine state
            let pointer = ctx.input(|i| i.pointer.clone());
            if let Some(pos) = pointer.latest_pos() {
                let logical = LogicalPoint::new(pos.x as f64, pos.y as f64);
                engine.on_mouse_move(logical);
                if pointer.is_decidedly_dragging() {
                    engine.on_mouse_drag(logical);
                }
            }
            if pointer.any_pressed() {
                if let Some(pos) = pointer.press_origin() {
                    engine.on_mouse_down(LogicalPoint::new(pos.x as f64, pos.y as f64));
                }
            }
            if pointer.any_released() {
                if let Some(pos) = pointer.latest_pos() {
                    engine.on_mouse_up("primary".into(), LogicalPoint::new(pos.x as f64, pos.y as f64));
                }
            }

            match &mut engine.state {
                crate::core::engine::EngineState::OverlayRunning => {
                    // Free selection drag transition handled above
                }
                crate::core::engine::EngineState::Editing => {
                    // Draw selection mask (40% black outside selection)
                    if let Some(sel) = engine.editor.selection {
                        let sel_rect = egui_rect_from_logical(sel);
                        let top = EguiRect::from_min_max(rect.min, egui::pos2(rect.max.x, sel_rect.min.y));
                        let bottom = EguiRect::from_min_max(egui::pos2(rect.min.x, sel_rect.max.y), rect.max);
                        let left = EguiRect::from_min_max(rect.min, egui::pos2(sel_rect.min.x, sel_rect.max.y));
                        let right = EguiRect::from_min_max(egui::pos2(sel_rect.max.x, sel_rect.min.y), rect.max);
                        for r in [top, bottom, left, right] {
                            if r.is_positive() {
                                ui.painter().rect_filled(r, Rounding::ZERO, Color32::from_black_alpha(102));
                            }
                        }
                    }
                    // Toolbar and layers rendered separately via other modules
                }
                _ => {}
            }
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn egui_rect_from_logical(r: Rect) -> EguiRect {
    EguiRect::from_min_max(
        egui::pos2(r.x as f32, r.y as f32),
        egui::pos2((r.x + r.w) as f32, (r.y + r.h) as f32),
    )
}
