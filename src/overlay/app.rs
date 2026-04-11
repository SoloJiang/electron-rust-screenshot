use crate::core::engine::Engine;
use crate::core::types::{Color, LogicalPoint, Rect};
use crate::overlay::toolbar::draw_toolbar;
use egui::{Color32, Rect as EguiRect, Rounding, Stroke};
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

    pub fn update(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut egui::Frame,
        frame_time_ms: f64,
        egui_paint_ms: f64,
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

                    // Toolbar
                    egui::TopBottomPanel::bottom("toolbar")
                        .frame(egui::Frame::window(&egui::Style::default()))
                        .show_inside(ui, |ui| {
                            draw_toolbar(ui, &mut engine.editor);
                        });

                    // Draw layers
                    for layer in &engine.editor.layers {
                        match layer {
                            crate::core::editor::Layer::ShapeRect { rect: r, stroke_width, color, .. } => {
                                let er = egui_rect_from_logical(*r);
                                ui.painter().rect_stroke(
                                    er,
                                    Rounding::ZERO,
                                    Stroke::new(*stroke_width, color32_from_color(*color)),
                                );
                            }
                            crate::core::editor::Layer::ShapeEllipse { rect: r, stroke_width, color, .. } => {
                                let er = egui_rect_from_logical(*r);
                                let center = er.center();
                                let radius = (er.width() + er.height()) / 4.0;
                                ui.painter().circle_stroke(
                                    center,
                                    radius,
                                    Stroke::new(*stroke_width, color32_from_color(*color)),
                                );
                            }
                            crate::core::editor::Layer::Arrow { start, end, stroke_width, color, .. } => {
                                let s = egui::pos2(start.x as f32, start.y as f32);
                                let e = egui::pos2(end.x as f32, end.y as f32);
                                ui.painter().line_segment([s, e], Stroke::new(*stroke_width, color32_from_color(*color)));
                                // Simple arrowhead (stub)
                            }
                            crate::core::editor::Layer::BrushPath { points, stroke_width, color, .. } => {
                                if points.len() >= 2 {
                                    let pts: Vec<egui::Pos2> = points
                                        .iter()
                                        .map(|p| egui::pos2(p.x as f32, p.y as f32))
                                        .collect();
                                    ui.painter().line(
                                        pts,
                                        Stroke::new(*stroke_width, color32_from_color(*color)),
                                    );
                                }
                            }
                            crate::core::editor::Layer::MosaicPath { .. } => {
                                // Mosaic is applied during composite/save; no live preview needed here
                            }
                            crate::core::editor::Layer::Text { pos, text, font_size, color, .. } => {
                                let p = egui::pos2(pos.x as f32, pos.y as f32);
                                ui.painter().text(
                                    p,
                                    egui::Align2::LEFT_TOP,
                                    text,
                                    egui::FontId::proportional(*font_size),
                                    color32_from_color(*color),
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        });

        if engine.show_debug_hud {
            let metrics = engine.perf.build_payload(0.0, 0.0, frame_time_ms, egui_paint_ms);
            engine.last_metrics = Some(metrics);
            if let Some(m) = engine.last_metrics {
                egui::Window::new("Debug HUD")
                    .collapsible(false)
                    .title_bar(false)
                    .anchor(egui::Align2::LEFT_TOP, egui::vec2(10.0, 10.0))
                    .show(ctx, |ui| {
                        ui.label(format!("Frame: {:.2}ms", m.frame_time_ms));
                        ui.label(format!("Paint: {:.2}ms", m.egui_paint_ms));
                        ui.label(format!("Capture: {:.2}ms", m.capture_ms));
                        ui.label(format!("Hit P99: {:.2}ms", m.hit_test_p99_ms));
                        ui.label(format!("Mem: {:.1}MB", m.memory_mb));
                    });
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn egui_rect_from_logical(r: Rect) -> EguiRect {
    EguiRect::from_min_max(
        egui::pos2(r.x as f32, r.y as f32),
        egui::pos2((r.x + r.w) as f32, (r.y + r.h) as f32),
    )
}

fn color32_from_color(c: Color) -> Color32 {
    Color32::from_rgba_premultiplied(c.r, c.g, c.b, c.a)
}
