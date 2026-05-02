use super::types::{Color, LogicalPoint, Rect};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tool {
    Select,
    Rect,
    Ellipse,
    Arrow,
    Brush,
    Mosaic,
    Text,
}

impl Tool {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Select" => Some(Tool::Select),
            "Rect" => Some(Tool::Rect),
            "Ellipse" => Some(Tool::Ellipse),
            "Arrow" => Some(Tool::Arrow),
            "Brush" => Some(Tool::Brush),
            "Mosaic" => Some(Tool::Mosaic),
            "Text" => Some(Tool::Text),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Layer {
    ShapeRect {
        id: String,
        rect: Rect,
        stroke_width: f32,
        color: Color,
    },
    ShapeEllipse {
        id: String,
        rect: Rect,
        stroke_width: f32,
        color: Color,
    },
    Arrow {
        id: String,
        start: LogicalPoint,
        end: LogicalPoint,
        stroke_width: f32,
        color: Color,
    },
    BrushPath {
        id: String,
        points: Vec<LogicalPoint>,
        stroke_width: f32,
        color: Color,
    },
    MosaicPath {
        id: String,
        points: Vec<LogicalPoint>,
        block_size: f32,
    },
    Text {
        id: String,
        pos: LogicalPoint,
        text: String,
        font_size: f32,
        color: Color,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayerOp {
    AddLayer {
        layer: Layer,
    },
    DeleteLayer {
        layer: Layer,
    },
    UpdateLayer {
        id: String,
        old: Layer,
        new: Layer,
    },
    UpdateSelectionAndLayers {
        old_selection: Option<Rect>,
        new_selection: Option<Rect>,
        old_layers: Vec<Layer>,
        new_layers: Vec<Layer>,
    },
}

pub struct EditorState {
    pub selection: Option<Rect>,
    pub layers: Vec<Layer>,
    pub undo_stack: Vec<LayerOp>,
    pub redo_stack: Vec<LayerOp>,
    pub active_tool: Tool,
    pub tool_color: Color,
    pub tool_size: f32,
    pub mosaic_block_size: f32,
    pub preview: Option<Layer>,
}

impl EditorState {
    pub fn new(default_color: Color, default_size: f32, mosaic_block_size: f32) -> Self {
        Self {
            selection: None,
            layers: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            active_tool: Tool::Select,
            tool_color: default_color,
            tool_size: default_size,
            mosaic_block_size,
            preview: None,
        }
    }

    pub fn add_layer(&mut self, layer: Layer) {
        self.undo_stack.push(LayerOp::AddLayer {
            layer: layer.clone(),
        });
        self.layers.push(layer);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(op) = self.undo_stack.pop() {
            match &op {
                LayerOp::AddLayer { layer } => {
                    if let Some(pos) = self
                        .layers
                        .iter()
                        .position(|l| layer_id(l) == layer_id(layer))
                    {
                        let removed = self.layers.remove(pos);
                        self.redo_stack.push(LayerOp::AddLayer { layer: removed });
                    }
                }
                LayerOp::DeleteLayer { layer } => {
                    self.layers.push(layer.clone());
                    self.redo_stack.push(LayerOp::DeleteLayer {
                        layer: layer.clone(),
                    });
                }
                LayerOp::UpdateLayer { id, old, .. } => {
                    if let Some(pos) = self.layers.iter().position(|l| layer_id(l) == *id) {
                        self.layers[pos] = old.clone();
                        self.redo_stack.push(op);
                    }
                }
                LayerOp::UpdateSelectionAndLayers {
                    old_selection,
                    old_layers,
                    ..
                } => {
                    self.selection = *old_selection;
                    self.layers = old_layers.clone();
                    self.redo_stack.push(op);
                }
            }
        }
    }

    pub fn redo(&mut self) {
        if let Some(op) = self.redo_stack.pop() {
            match &op {
                LayerOp::AddLayer { layer } => {
                    self.layers.push(layer.clone());
                    self.undo_stack.push(LayerOp::AddLayer {
                        layer: layer.clone(),
                    });
                }
                LayerOp::DeleteLayer { layer } => {
                    if let Some(pos) = self
                        .layers
                        .iter()
                        .position(|l| layer_id(l) == layer_id(layer))
                    {
                        let removed = self.layers.remove(pos);
                        self.undo_stack
                            .push(LayerOp::DeleteLayer { layer: removed });
                    }
                }
                LayerOp::UpdateLayer { id, new, .. } => {
                    if let Some(pos) = self.layers.iter().position(|l| layer_id(l) == *id) {
                        let old = self.layers[pos].clone();
                        self.layers[pos] = new.clone();
                        self.undo_stack.push(LayerOp::UpdateLayer {
                            id: id.clone(),
                            old,
                            new: new.clone(),
                        });
                    }
                }
                LayerOp::UpdateSelectionAndLayers {
                    new_selection,
                    new_layers,
                    ..
                } => {
                    self.selection = *new_selection;
                    self.layers = new_layers.clone();
                    self.undo_stack.push(op);
                }
            }
        }
    }

    pub fn clear_preview(&mut self) {
        self.preview = None;
    }

    pub fn transform_selection(
        original_selection: Rect,
        layers: &[Layer],
        kind: &super::types::ResizeHit,
        delta: LogicalPoint,
    ) -> (Rect, Vec<Layer>) {
        use super::types::{Corner, Edge, ResizeHit};
        const MIN_SIZE: f64 = 8.0;

        let mut new_rect = original_selection;

        match kind {
            ResizeHit::Move => {
                new_rect.x += delta.x;
                new_rect.y += delta.y;
            }
            ResizeHit::ResizeEdge { edge } => match edge {
                Edge::North => {
                    new_rect.y += delta.y;
                    new_rect.h -= delta.y;
                    if new_rect.h < MIN_SIZE {
                        let extra = MIN_SIZE - new_rect.h;
                        new_rect.y -= extra;
                        new_rect.h = MIN_SIZE;
                    }
                }
                Edge::South => {
                    new_rect.h += delta.y;
                    if new_rect.h < MIN_SIZE {
                        new_rect.h = MIN_SIZE;
                    }
                }
                Edge::West => {
                    new_rect.x += delta.x;
                    new_rect.w -= delta.x;
                    if new_rect.w < MIN_SIZE {
                        let extra = MIN_SIZE - new_rect.w;
                        new_rect.x -= extra;
                        new_rect.w = MIN_SIZE;
                    }
                }
                Edge::East => {
                    new_rect.w += delta.x;
                    if new_rect.w < MIN_SIZE {
                        new_rect.w = MIN_SIZE;
                    }
                }
            },
            ResizeHit::ResizeCorner { corner } => match corner {
                Corner::NW => {
                    new_rect.x += delta.x;
                    new_rect.y += delta.y;
                    new_rect.w -= delta.x;
                    new_rect.h -= delta.y;
                    if new_rect.w < MIN_SIZE {
                        let extra = MIN_SIZE - new_rect.w;
                        new_rect.x -= extra;
                        new_rect.w = MIN_SIZE;
                    }
                    if new_rect.h < MIN_SIZE {
                        let extra = MIN_SIZE - new_rect.h;
                        new_rect.y -= extra;
                        new_rect.h = MIN_SIZE;
                    }
                }
                Corner::NE => {
                    new_rect.y += delta.y;
                    new_rect.w += delta.x;
                    new_rect.h -= delta.y;
                    if new_rect.w < MIN_SIZE {
                        new_rect.w = MIN_SIZE;
                    }
                    if new_rect.h < MIN_SIZE {
                        let extra = MIN_SIZE - new_rect.h;
                        new_rect.y -= extra;
                        new_rect.h = MIN_SIZE;
                    }
                }
                Corner::SW => {
                    new_rect.x += delta.x;
                    new_rect.w -= delta.x;
                    new_rect.h += delta.y;
                    if new_rect.w < MIN_SIZE {
                        let extra = MIN_SIZE - new_rect.w;
                        new_rect.x -= extra;
                        new_rect.w = MIN_SIZE;
                    }
                    if new_rect.h < MIN_SIZE {
                        new_rect.h = MIN_SIZE;
                    }
                }
                Corner::SE => {
                    new_rect.w += delta.x;
                    new_rect.h += delta.y;
                    if new_rect.w < MIN_SIZE {
                        new_rect.w = MIN_SIZE;
                    }
                    if new_rect.h < MIN_SIZE {
                        new_rect.h = MIN_SIZE;
                    }
                }
            },
        }

        let sx = if original_selection.w == 0.0 {
            0.0
        } else {
            new_rect.w / original_selection.w
        };
        let sy = if original_selection.h == 0.0 {
            0.0
        } else {
            new_rect.h / original_selection.h
        };
        let new_layers: Vec<Layer> = layers
            .iter()
            .map(|l| transform_layer(l, original_selection, new_rect, sx, sy))
            .collect();

        (new_rect, new_layers)
    }
}

fn transform_layer(layer: &Layer, original: Rect, new_rect: Rect, sx: f64, sy: f64) -> Layer {
    let norm_x = |x: f64| -> f64 {
        if original.w == 0.0 {
            0.0
        } else {
            (x - original.x) / original.w
        }
    };
    let norm_y = |y: f64| -> f64 {
        if original.h == 0.0 {
            0.0
        } else {
            (y - original.y) / original.h
        }
    };
    let denorm_x = |nx: f64| -> f64 { new_rect.x + nx * new_rect.w };
    let denorm_y = |ny: f64| -> f64 { new_rect.y + ny * new_rect.h };

    match layer {
        Layer::ShapeRect {
            id,
            rect,
            stroke_width,
            color,
        } => Layer::ShapeRect {
            id: id.clone(),
            rect: Rect {
                x: denorm_x(norm_x(rect.x)),
                y: denorm_y(norm_y(rect.y)),
                w: rect.w * sx,
                h: rect.h * sy,
            },
            stroke_width: *stroke_width,
            color: *color,
        },
        Layer::ShapeEllipse {
            id,
            rect,
            stroke_width,
            color,
        } => Layer::ShapeEllipse {
            id: id.clone(),
            rect: Rect {
                x: denorm_x(norm_x(rect.x)),
                y: denorm_y(norm_y(rect.y)),
                w: rect.w * sx,
                h: rect.h * sy,
            },
            stroke_width: *stroke_width,
            color: *color,
        },
        Layer::Arrow {
            id,
            start,
            end,
            stroke_width,
            color,
        } => Layer::Arrow {
            id: id.clone(),
            start: LogicalPoint {
                x: denorm_x(norm_x(start.x)),
                y: denorm_y(norm_y(start.y)),
            },
            end: LogicalPoint {
                x: denorm_x(norm_x(end.x)),
                y: denorm_y(norm_y(end.y)),
            },
            stroke_width: *stroke_width,
            color: *color,
        },
        Layer::BrushPath {
            id,
            points,
            stroke_width,
            color,
        } => Layer::BrushPath {
            id: id.clone(),
            points: points
                .iter()
                .map(|p| LogicalPoint {
                    x: denorm_x(norm_x(p.x)),
                    y: denorm_y(norm_y(p.y)),
                })
                .collect(),
            stroke_width: *stroke_width,
            color: *color,
        },
        Layer::MosaicPath {
            id,
            points,
            block_size,
        } => Layer::MosaicPath {
            id: id.clone(),
            points: points
                .iter()
                .map(|p| LogicalPoint {
                    x: denorm_x(norm_x(p.x)),
                    y: denorm_y(norm_y(p.y)),
                })
                .collect(),
            block_size: *block_size,
        },
        Layer::Text {
            id,
            pos,
            text,
            font_size,
            color,
        } => Layer::Text {
            id: id.clone(),
            pos: LogicalPoint {
                x: denorm_x(norm_x(pos.x)),
                y: denorm_y(norm_y(pos.y)),
            },
            text: text.clone(),
            font_size: *font_size,
            color: *color,
        },
    }
}

fn layer_id(layer: &Layer) -> String {
    match layer {
        Layer::ShapeRect { id, .. } => id.clone(),
        Layer::ShapeEllipse { id, .. } => id.clone(),
        Layer::Arrow { id, .. } => id.clone(),
        Layer::BrushPath { id, .. } => id.clone(),
        Layer::MosaicPath { id, .. } => id.clone(),
        Layer::Text { id, .. } => id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn add_and_undo_layer() {
        let mut state = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
        let layer = Layer::ShapeRect {
            id: Uuid::new_v4().to_string(),
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        };
        state.add_layer(layer);
        assert_eq!(state.layers.len(), 1);
        state.undo();
        assert!(state.layers.is_empty());
    }

    #[test]
    fn undo_redo_update_layer() {
        let mut state = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
        let id = Uuid::new_v4().to_string();
        let layer = Layer::ShapeRect {
            id: id.clone(),
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        };
        state.add_layer(layer.clone());

        let updated = Layer::ShapeRect {
            id: id.clone(),
            rect: Rect::new(0.0, 0.0, 200.0, 200.0),
            stroke_width: 4.0,
            color: Color::new(0, 255, 0, 255),
        };
        state.layers[0] = updated.clone();
        state.undo_stack.push(LayerOp::UpdateLayer {
            id: id.clone(),
            old: layer.clone(),
            new: updated.clone(),
        });

        state.undo();
        assert_eq!(state.layers[0], layer);

        state.redo();
        assert_eq!(state.layers[0], updated);
    }

    #[test]
    fn transform_selection_move_preserves_relative_positions() {
        let original = Rect::new(0.0, 0.0, 100.0, 100.0);
        let layers = vec![
            Layer::ShapeRect {
                id: Uuid::new_v4().to_string(),
                rect: Rect::new(10.0, 10.0, 20.0, 20.0),
                stroke_width: 2.0,
                color: Color::new(255, 0, 0, 255),
            },
            Layer::Arrow {
                id: Uuid::new_v4().to_string(),
                start: LogicalPoint::new(0.0, 0.0),
                end: LogicalPoint::new(100.0, 100.0),
                stroke_width: 2.0,
                color: Color::new(255, 0, 0, 255),
            },
        ];

        let (new_rect, new_layers) = EditorState::transform_selection(
            original,
            &layers,
            &crate::core::types::ResizeHit::Move,
            LogicalPoint::new(10.0, 20.0),
        );

        assert_eq!(new_rect, Rect::new(10.0, 20.0, 100.0, 100.0));

        if let Layer::ShapeRect { rect, .. } = &new_layers[0] {
            assert_eq!(*rect, Rect::new(20.0, 30.0, 20.0, 20.0));
        } else {
            panic!("expected ShapeRect");
        }

        if let Layer::Arrow { start, end, .. } = &new_layers[1] {
            assert_eq!(*start, LogicalPoint::new(10.0, 20.0));
            assert_eq!(*end, LogicalPoint::new(110.0, 120.0));
        } else {
            panic!("expected Arrow");
        }
    }

    #[test]
    fn transform_selection_scale_scales_layers() {
        let original = Rect::new(0.0, 0.0, 100.0, 100.0);
        let layers = vec![Layer::ShapeRect {
            id: Uuid::new_v4().to_string(),
            rect: Rect::new(10.0, 0.0, 20.0, 50.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        }];

        let (new_rect, new_layers) = EditorState::transform_selection(
            original,
            &layers,
            &crate::core::types::ResizeHit::ResizeEdge {
                edge: crate::core::types::Edge::East,
            },
            LogicalPoint::new(100.0, 0.0),
        );

        assert_eq!(new_rect, Rect::new(0.0, 0.0, 200.0, 100.0));

        if let Layer::ShapeRect { rect, .. } = &new_layers[0] {
            assert_eq!(*rect, Rect::new(20.0, 0.0, 40.0, 50.0));
        } else {
            panic!("expected ShapeRect");
        }
    }

    #[test]
    fn undo_redo_update_selection_and_layers() {
        let mut state = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
        let old_selection = Some(Rect::new(0.0, 0.0, 100.0, 100.0));
        let new_selection = Some(Rect::new(10.0, 10.0, 200.0, 200.0));
        let id = Uuid::new_v4().to_string();
        let old_layers = vec![Layer::ShapeRect {
            id: id.clone(),
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        }];
        let new_layers = vec![Layer::ShapeRect {
            id: id.clone(),
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            stroke_width: 4.0,
            color: Color::new(0, 255, 0, 255),
        }];

        state.selection = new_selection;
        state.layers = new_layers.clone();
        state.undo_stack.push(LayerOp::UpdateSelectionAndLayers {
            old_selection,
            new_selection,
            old_layers: old_layers.clone(),
            new_layers: new_layers.clone(),
        });

        state.undo();
        assert_eq!(state.selection, old_selection);
        assert_eq!(state.layers, old_layers);

        state.redo();
        assert_eq!(state.selection, new_selection);
        assert_eq!(state.layers, new_layers);
    }

    #[test]
    fn transform_selection_clamps_min_size() {
        let original = Rect::new(100.0, 100.0, 20.0, 20.0);
        let layers: Vec<Layer> = Vec::new();

        // Drag North edge down by 50px (would make height negative)
        let (new_rect, _) = EditorState::transform_selection(
            original,
            &layers,
            &crate::core::types::ResizeHit::ResizeEdge {
                edge: crate::core::types::Edge::North,
            },
            LogicalPoint::new(0.0, 50.0),
        );
        assert_eq!(new_rect.h, 8.0);
        assert_eq!(new_rect.y, 100.0 + 20.0 - 8.0);

        // Drag West edge right by 50px (would make width negative)
        let (new_rect, _) = EditorState::transform_selection(
            original,
            &layers,
            &crate::core::types::ResizeHit::ResizeEdge {
                edge: crate::core::types::Edge::West,
            },
            LogicalPoint::new(50.0, 0.0),
        );
        assert_eq!(new_rect.w, 8.0);
        assert_eq!(new_rect.x, 100.0 + 20.0 - 8.0);
    }

    #[test]
    fn transform_selection_corner_se_scales_both_dimensions() {
        let original = Rect::new(0.0, 0.0, 100.0, 100.0);
        let id = Uuid::new_v4().to_string();
        let layers = vec![Layer::ShapeRect {
            id: id.clone(),
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        }];

        let (new_rect, new_layers) = EditorState::transform_selection(
            original,
            &layers,
            &crate::core::types::ResizeHit::ResizeCorner {
                corner: crate::core::types::Corner::SE,
            },
            LogicalPoint::new(100.0, 100.0),
        );

        assert_eq!(new_rect, Rect::new(0.0, 0.0, 200.0, 200.0));

        if let Layer::ShapeRect { rect, .. } = &new_layers[0] {
            assert_eq!(*rect, Rect::new(20.0, 20.0, 40.0, 40.0));
        } else {
            panic!("expected ShapeRect");
        }
    }
}
