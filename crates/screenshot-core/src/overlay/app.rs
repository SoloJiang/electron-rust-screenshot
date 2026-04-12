use crate::core::capture::ScreenFrame;
use crate::core::engine::Engine;
use crate::core::types::{Color, Corner, Edge, LogicalPoint, Rect, ResizeHit};
use crate::overlay::toolbar::draw_toolbar;
use egui::{Color32, Rect as EguiRect, Rounding, Stroke};
use std::sync::Arc;
use parking_lot::Mutex;

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
        show_toolbar: bool,
    ) {
        let mut engine = self.engine.lock();

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
            if pointer.button_double_clicked(egui::PointerButton::Primary) {
                engine.select_hovered_window();
            }

            match &mut engine.state {
                crate::core::engine::EngineState::OverlayRunning => {
                    // Dark mask over everything
                    ui.painter().rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(120));

                    if let Some(win) = &engine.hovered_window {
                        const STROKE: f64 = 2.0;
                        self.draw_unmasked_region(ui.painter(), inset_rect(win.bounds, STROKE), offset);
                        let clip = egui_rect_from_logical(win.bounds, offset);
                        let stroke_rect = egui_rect_from_logical(inset_rect(win.bounds, STROKE * 0.5), offset);
                        ui.painter()
                            .with_clip_rect(clip)
                            .rect_stroke(
                                stroke_rect,
                                Rounding::ZERO,
                                Stroke::new(STROKE as f32, Color32::from_rgb(0, 120, 255)),
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
                    const STROKE: f64 = 2.0;
                    self.draw_unmasked_region(ui.painter(), inset_rect(sel, STROKE), offset);
                    let clip = egui_rect_from_logical(sel, offset);
                    let stroke_rect = egui_rect_from_logical(inset_rect(sel, STROKE * 0.5), offset);
                    ui.painter()
                        .with_clip_rect(clip)
                        .rect_stroke(
                            stroke_rect,
                            Rounding::ZERO,
                            Stroke::new(STROKE as f32, Color32::from_rgb(0, 120, 255)),
                        );
                }
                crate::core::engine::EngineState::Editing => {
                    if let Some(sel) = engine.editor.selection {
                        let logical_pos = pointer.latest_pos().map(|pos| {
                            LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y)
                        });
                        let resize_hit = logical_pos.and_then(|p| sel.hit_test_resize_handle(p, 8.0));

                        let in_selection = logical_pos.map(|p| sel.contains(p)).unwrap_or(true);
                        let in_transform_zone = resize_hit.is_some();

                        let can_move = engine.editor.active_tool == crate::core::editor::Tool::Select;

                        // Event routing
                        if engine.selection_transform.is_some() {
                            if pointer.is_decidedly_dragging() {
                                if let Some(pos) = logical_pos {
                                    engine.on_selection_transform_drag(pos);
                                }
                            }
                            if pointer.any_released() {
                                if let Some(pos) = logical_pos {
                                    engine.on_selection_transform_end(pos);
                                }
                            }
                        } else if in_selection || in_transform_zone {
                            if pointer.any_pressed() {
                                if let Some(pos) = logical_pos {
                                    match resize_hit {
                                        Some(ResizeHit::Move) if can_move => {
                                            engine.on_edit_mouse_down(pos);
                                        }
                                        Some(ResizeHit::Move) => {
                                            if sel.contains(pos) {
                                                engine.on_edit_mouse_down(pos);
                                            }
                                        }
                                        Some(hit) => {
                                            engine.on_selection_transform_start(pos, hit);
                                        }
                                        None => {
                                            if sel.contains(pos) {
                                                engine.on_edit_mouse_down(pos);
                                            }
                                        }
                                    }
                                }
                            }
                            if pointer.is_decidedly_dragging() {
                                if let Some(pos) = logical_pos {
                                    let is_move_zone = resize_hit == Some(ResizeHit::Move);
                                    if engine.edit_drag_start.is_some() && is_move_zone && can_move {
                                        engine.abort_edit_drag();
                                        engine.on_selection_transform_start(pos, ResizeHit::Move);
                                    } else {
                                        engine.on_edit_mouse_drag(pos);
                                    }
                                }
                            }
                            if pointer.any_released() {
                                if let Some(pos) = logical_pos {
                                    engine.on_edit_mouse_up(pos);
                                }
                            }
                        }

                        // Cursor feedback
                        if let Some(ref state) = engine.selection_transform {
                            ui.ctx().set_cursor_icon(match state.kind {
                                ResizeHit::Move => {
                                    if can_move {
                                        egui::CursorIcon::Move
                                    } else {
                                        egui::CursorIcon::Default
                                    }
                                }
                                ResizeHit::ResizeEdge { edge: Edge::North | Edge::South } => {
                                    egui::CursorIcon::ResizeVertical
                                }
                                ResizeHit::ResizeEdge { edge: Edge::East | Edge::West } => {
                                    egui::CursorIcon::ResizeHorizontal
                                }
                                ResizeHit::ResizeCorner { corner: Corner::NW | Corner::SE } => {
                                    egui::CursorIcon::ResizeNwSe
                                }
                                ResizeHit::ResizeCorner { corner: Corner::NE | Corner::SW } => {
                                    egui::CursorIcon::ResizeNeSw
                                }
                            });
                        } else if let Some(hit) = resize_hit {
                            ui.ctx().set_cursor_icon(match hit {
                                ResizeHit::Move => {
                                    if can_move {
                                        egui::CursorIcon::Move
                                    } else {
                                        egui::CursorIcon::Default
                                    }
                                }
                                ResizeHit::ResizeEdge { edge: Edge::North | Edge::South } => {
                                    egui::CursorIcon::ResizeVertical
                                }
                                ResizeHit::ResizeEdge { edge: Edge::East | Edge::West } => {
                                    egui::CursorIcon::ResizeHorizontal
                                }
                                ResizeHit::ResizeCorner { corner: Corner::NW | Corner::SE } => {
                                    egui::CursorIcon::ResizeNwSe
                                }
                                ResizeHit::ResizeCorner { corner: Corner::NE | Corner::SW } => {
                                    egui::CursorIcon::ResizeNeSw
                                }
                            });
                        } else {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                        }

                        let screen_rect = Rect::new(offset.x, offset.y, rect.width() as f64, rect.height() as f64);
                        if sel.intersects(screen_rect) {
                            // Uniform mask over entire screen
                            ui.painter().rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(120));

                            // Unmask the selected region (inset by border width for border-box look)
                            const STROKE: f64 = 2.0;
                            self.draw_unmasked_region(ui.painter(), inset_rect(sel, STROKE), offset);
                            // Blue selection border (same as hover/window-select color)
                            let clip = egui_rect_from_logical(sel, offset);
                            let sel_rect = egui_rect_from_logical(inset_rect(sel, STROKE * 0.5), offset);
                            ui.painter()
                                .with_clip_rect(clip)
                                .rect_stroke(
                                    sel_rect,
                                    Rounding::ZERO,
                                    Stroke::new(STROKE as f32, Color32::from_rgb(0, 120, 255)),
                                );

                            // Draw 8 resize handles
                            let handle_radius = 4.0;
                            let handle_stroke = Stroke::new(2.0, Color32::from_rgb(0, 120, 255));
                            let handle_positions = [
                                (sel.x, sel.y),                         // NW
                                (sel.x + sel.w / 2.0, sel.y),           // N
                                (sel.x + sel.w, sel.y),                 // NE
                                (sel.x + sel.w, sel.y + sel.h / 2.0),   // E
                                (sel.x + sel.w, sel.y + sel.h),         // SE
                                (sel.x + sel.w / 2.0, sel.y + sel.h),   // S
                                (sel.x, sel.y + sel.h),                 // SW
                                (sel.x, sel.y + sel.h / 2.0),           // W
                            ];
                            for (hx, hy) in handle_positions {
                                let hp = egui::pos2((hx - offset.x) as f32, (hy - offset.y) as f32);
                                ui.painter().circle_filled(hp, handle_radius, Color32::WHITE);
                                ui.painter().circle_stroke(hp, handle_radius, handle_stroke);
                            }

                            // Toolbar as a floating window centered below the selection
                            if show_toolbar {
                                let toolbar_pos = egui::pos2(
                                    (sel.x + sel.w / 2.0 - offset.x) as f32,
                                    (sel.y + sel.h - offset.y + 8.0) as f32,
                                );
                                let save_clicked = egui::Area::new(egui::Id::new("screenshot_toolbar"))
                                    .pivot(egui::Align2::CENTER_TOP)
                                    .fixed_pos(toolbar_pos)
                                    .show(ctx, |ui| {
                                        egui::Frame::window(&egui::Style::default())
                                            .show(ui, |ui| draw_toolbar(ui, &mut engine.editor))
                                            .inner
                                    })
                                    .inner;
                                if save_clicked {
                                    engine.save();
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
                        } else {
                            ui.painter().rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(120));
                        }
                    } else {
                        ui.painter().rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(120));
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

fn inset_rect(r: Rect, inset: f64) -> Rect {
    Rect::new(
        r.x + inset,
        r.y + inset,
        (r.w - 2.0 * inset).max(0.0),
        (r.h - 2.0 * inset).max(0.0),
    )
}

fn color32_from_color(c: Color) -> Color32 {
    Color32::from_rgba_premultiplied(c.r, c.g, c.b, c.a)
}
