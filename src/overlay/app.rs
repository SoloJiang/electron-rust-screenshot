use crate::core::capture::ScreenFrame;
use crate::core::engine::Engine;
use crate::core::types::{Color, LogicalPoint, Rect};
use crate::overlay::toolbar::draw_toolbar;
use egui::{Color32, Rect as EguiRect, Rounding, Stroke};
use std::sync::{Arc, Mutex};

pub struct ScreenshotApp {
    pub engine: Arc<Mutex<Engine>>,
    pub frame_textures: Vec<Option<egui::TextureHandle>>,
    pub frames: Vec<ScreenFrame>,
}

impl ScreenshotApp {
    pub fn new(engine: Arc<Mutex<Engine>>, frames: Vec<ScreenFrame>) -> Self {
        Self {
            engine,
            frame_textures: vec![None; frames.len()],
            frames,
        }
    }

    pub fn load_screenshot_textures(&mut self, ctx: &egui::Context) {
        for (i, frame) in self.frames.iter().enumerate() {
            let img = &frame.image;
            let width = img.width() as usize;
            let height = img.height() as usize;
            let pixels = img.as_raw();
            let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], pixels);
            self.frame_textures[i] = Some(ctx.load_texture(
                &format!("screenshot-{}", frame.screen_id),
                color_image,
                Default::default(),
            ));
        }
    }

    fn draw_screenshot_textures(&self, painter: &egui::Painter, offset: LogicalPoint) {
        for (frame, tex_opt) in self.frames.iter().zip(self.frame_textures.iter()) {
            if let Some(tex) = tex_opt {
                let b = frame.logical_bounds;
                let r = egui::Rect::from_min_max(
                    egui::pos2((b.x - offset.x) as f32, (b.y - offset.y) as f32),
                    egui::pos2((b.x + b.w - offset.x) as f32, (b.y + b.h - offset.y) as f32),
                );
                painter.image(
                    tex.id(),
                    r,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
    }

    fn draw_unmasked_region(&self, painter: &egui::Painter, region: Rect, offset: LogicalPoint) {
        let clip = egui::Rect::from_min_max(
            egui::pos2((region.x - offset.x) as f32, (region.y - offset.y) as f32),
            egui::pos2((region.x + region.w - offset.x) as f32, (region.y + region.h - offset.y) as f32),
        );
        let clipped = painter.with_clip_rect(clip);
        self.draw_screenshot_textures(&clipped, offset);
    }

    pub fn update(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut egui::Frame,
        offset: LogicalPoint,
        frame_time_ms: f64,
        egui_paint_ms: f64,
    ) {
        let mut engine = self.engine.lock().unwrap();

        let panel = egui::CentralPanel::default().frame(egui::Frame::none());
        panel.show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();

            // 1. Draw background screenshot textures
            self.draw_screenshot_textures(ui.painter(), offset);

            // 2. Handle mouse interaction based on engine state
            let pointer = ctx.input(|i| i.pointer.clone());
            if let Some(pos) = pointer.latest_pos() {
                let logical = LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y);
                engine.on_mouse_move(logical);
                if pointer.is_decidedly_dragging() {
                    engine.on_mouse_drag(logical);
                }
            }
            if pointer.any_pressed() {
                if let Some(pos) = pointer.press_origin() {
                    engine.on_mouse_down(LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y));
                }
            }
            if pointer.any_released() {
                if let Some(pos) = pointer.latest_pos() {
                    engine.on_mouse_up(
                        LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y),
                    );
                }
            }

            match &mut engine.state {
                crate::core::engine::EngineState::OverlayRunning => {
                    // Dark mask over everything
                    ui.painter().rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(120));

                    if let Some(win) = &engine.hovered_window {
                        // Unmask the hovered window so original screenshot shows through
                        self.draw_unmasked_region(ui.painter(), win.bounds, offset);
                        let r = egui_rect_from_logical(win.bounds, offset);
                        ui.painter().rect_stroke(
                            r,
                            Rounding::ZERO,
                            Stroke::new(2.0, Color32::from_rgb(0, 120, 255)),
                        );
                    }
                }
                crate::core::engine::EngineState::FreeSelecting { start, current } => {
                    ui.painter().rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(120));
                    let sel = Rect::new(
                        start.x.min(current.x),
                        start.y.min(current.y),
                        (current.x - start.x).abs(),
                        (current.y - start.y).abs(),
                    );
                    // Unmask the selection area
                    self.draw_unmasked_region(ui.painter(), sel, offset);
                    let s = egui::pos2((start.x - offset.x) as f32, (start.y - offset.y) as f32);
                    let c = egui::pos2((current.x - offset.x) as f32, (current.y - offset.y) as f32);
                    let r = EguiRect::from_two_pos(s, c);
                    ui.painter()
                        .rect_stroke(r, Rounding::ZERO, Stroke::new(1.0, Color32::WHITE));
                }
                crate::core::engine::EngineState::Editing => {
                    // Only allow editing mouse interaction inside the selection area.
                    // This effectively locks the other monitor(s) when editing a single-screen selection.
                    let in_selection = engine.editor.selection.map(|sel| {
                        pointer.latest_pos().map(|pos| {
                            let logical = LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y);
                            sel.contains(logical)
                        }).unwrap_or(false)
                    }).unwrap_or(true);

                    if in_selection {
                        if pointer.any_pressed() {
                            if let Some(pos) = pointer.press_origin() {
                                engine.on_edit_mouse_down(LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y));
                            }
                        }
                        if pointer.is_decidedly_dragging() {
                            if let Some(pos) = pointer.latest_pos() {
                                engine.on_edit_mouse_drag(LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y));
                            }
                        }
                        if pointer.any_released() {
                            if let Some(pos) = pointer.latest_pos() {
                                engine.on_edit_mouse_up(LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y));
                            }
                        }
                    }

                    // Uniform mask over entire screen
                    ui.painter().rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(120));

                    if let Some(sel) = engine.editor.selection {
                        // Unmask the selected region
                        self.draw_unmasked_region(ui.painter(), sel, offset);
                        // White selection border
                        let sel_rect = egui_rect_from_logical(sel, offset);
                        ui.painter().rect_stroke(
                            sel_rect,
                            Rounding::ZERO,
                            Stroke::new(1.0, Color32::WHITE),
                        );

                        // Toolbar as a floating window just below the selection so it stays
                        // visible regardless of the multi-monitor union rect size.
                        let toolbar_pos = egui::pos2(
                            (sel.x - offset.x) as f32,
                            (sel.y + sel.h - offset.y + 8.0) as f32,
                        );
                        let save_clicked = egui::Window::new("screenshot_toolbar")
                            .collapsible(false)
                            .title_bar(false)
                            .fixed_pos(toolbar_pos)
                            .auto_sized()
                            .frame(egui::Frame::window(&egui::Style::default()))
                            .show(ctx, |ui| draw_toolbar(ui, &mut engine.editor))
                            .and_then(|r| r.inner)
                            .unwrap_or(false);
                        if save_clicked {
                            let frames = &self.frames;
                            engine.save(frames);
                        }
                    }

                    // Draw committed layers
                    for layer in &engine.editor.layers {
                        draw_layer(ui.painter(), layer, offset);
                    }

                    // Draw preview layer
                    if let Some(preview) = &engine.editor.preview {
                        draw_layer(ui.painter(), preview, offset);
                    }
                }
                _ => {}
            }
        });

        if engine.show_debug_hud {
            let metrics = engine
                .perf
                .build_payload(0.0, 0.0, frame_time_ms, egui_paint_ms);
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

fn draw_layer(painter: &egui::Painter, layer: &crate::core::editor::Layer, offset: LogicalPoint) {
    match layer {
        crate::core::editor::Layer::ShapeRect {
            rect: r,
            stroke_width,
            color,
            ..
        } => {
            let er = egui_rect_from_logical(*r, offset);
            painter.rect_stroke(
                er,
                Rounding::ZERO,
                Stroke::new(*stroke_width, color32_from_color(*color)),
            );
        }
        crate::core::editor::Layer::ShapeEllipse {
            rect: r,
            stroke_width,
            color,
            ..
        } => {
            let er = egui_rect_from_logical(*r, offset);
            let center = er.center();
            let radius = (er.width() + er.height()) / 4.0;
            painter.circle_stroke(
                center,
                radius,
                Stroke::new(*stroke_width, color32_from_color(*color)),
            );
        }
        crate::core::editor::Layer::Arrow {
            start,
            end,
            stroke_width,
            color,
            ..
        } => {
            let s = egui::pos2((start.x - offset.x) as f32, (start.y - offset.y) as f32);
            let e = egui::pos2((end.x - offset.x) as f32, (end.y - offset.y) as f32);
            painter.line_segment(
                [s, e],
                Stroke::new(*stroke_width, color32_from_color(*color)),
            );

            // Arrowhead
            let dx = start.x - end.x;
            let dy = start.y - end.y;
            let len_sq = (dx * dx + dy * dy) as f32;
            if len_sq > 0.0 {
                let len = len_sq.sqrt();
                let ux = (dx as f32) / len;
                let uy = (dy as f32) / len;
                let wing_len = *stroke_width * 3.0;
                let cos30 = 0.8660254;
                let sin30 = 0.5;
                let w1x = ux * cos30 - uy * sin30;
                let w1y = ux * sin30 + uy * cos30;
                let w2x = ux * cos30 + uy * sin30;
                let w2y = -ux * sin30 + uy * cos30;
                let wing1 = egui::pos2(e.x + w1x * wing_len, e.y + w1y * wing_len);
                let wing2 = egui::pos2(e.x + w2x * wing_len, e.y + w2y * wing_len);
                let stroke = Stroke::new(*stroke_width, color32_from_color(*color));
                painter.line_segment([e, wing1], stroke);
                painter.line_segment([e, wing2], stroke);
            }
        }
        crate::core::editor::Layer::BrushPath {
            points,
            stroke_width,
            color,
            ..
        } => {
            if points.len() >= 2 {
                let pts: Vec<egui::Pos2> = points
                    .iter()
                    .map(|p| egui::pos2((p.x - offset.x) as f32, (p.y - offset.y) as f32))
                    .collect();
                painter.line(pts, Stroke::new(*stroke_width, color32_from_color(*color)));
            }
        }
        crate::core::editor::Layer::MosaicPath { .. } => {
            // Mosaic is applied during composite/save; no live preview needed here
        }
        crate::core::editor::Layer::Text {
            pos,
            text,
            font_size,
            color,
            ..
        } => {
            let p = egui::pos2((pos.x - offset.x) as f32, (pos.y - offset.y) as f32);
            painter.text(
                p,
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(*font_size),
                color32_from_color(*color),
            );
        }
    }
}

fn egui_rect_from_logical(r: Rect, offset: LogicalPoint) -> EguiRect {
    EguiRect::from_min_max(
        egui::pos2((r.x - offset.x) as f32, (r.y - offset.y) as f32),
        egui::pos2((r.x + r.w - offset.x) as f32, (r.y + r.h - offset.y) as f32),
    )
}

fn color32_from_color(c: Color) -> Color32 {
    Color32::from_rgba_premultiplied(c.r, c.g, c.b, c.a)
}
