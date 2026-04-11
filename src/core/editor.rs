use super::types::{Color, LogicalPoint, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Rect,
    Ellipse,
    Arrow,
    Brush,
    Mosaic,
    Text,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Layer {
    ShapeRect { id: String, rect: Rect, stroke_width: f32, color: Color },
    ShapeEllipse { id: String, rect: Rect, stroke_width: f32, color: Color },
    Arrow { id: String, start: LogicalPoint, end: LogicalPoint, stroke_width: f32, color: Color },
    BrushPath { id: String, points: Vec<LogicalPoint>, stroke_width: f32, color: Color },
    MosaicPath { id: String, points: Vec<LogicalPoint>, block_size: f32 },
    Text { id: String, pos: LogicalPoint, text: String, font_size: f32, color: Color },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayerOp {
    AddLayer { id: String },
    DeleteLayer { id: String },
    UpdateLayer { id: String, old: Layer, new: Layer },
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
            active_tool: Tool::Rect,
            tool_color: default_color,
            tool_size: default_size,
            mosaic_block_size,
            preview: None,
        }
    }

    pub fn add_layer(&mut self, layer: Layer) {
        let id = match &layer {
            Layer::ShapeRect { id, .. } => id.clone(),
            Layer::ShapeEllipse { id, .. } => id.clone(),
            Layer::Arrow { id, .. } => id.clone(),
            Layer::BrushPath { id, .. } => id.clone(),
            Layer::MosaicPath { id, .. } => id.clone(),
            Layer::Text { id, .. } => id.clone(),
        };
        self.layers.push(layer);
        self.undo_stack.push(LayerOp::AddLayer { id });
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(op) = self.undo_stack.pop() {
            match &op {
                LayerOp::AddLayer { id } => {
                    if let Some(pos) = self.layers.iter().position(|l| layer_id(l) == *id) {
                        let removed = self.layers.remove(pos);
                        self.redo_stack.push(LayerOp::DeleteLayer { id: layer_id(&removed) });
                    }
                }
                LayerOp::DeleteLayer { id: _ } => {
                    // re-insert logic omitted for brevity; implement if complex undo needed
                    self.redo_stack.push(op);
                }
                LayerOp::UpdateLayer { id, old, .. } => {
                    if let Some(pos) = self.layers.iter().position(|l| layer_id(l) == *id) {
                        self.layers[pos] = old.clone();
                        self.redo_stack.push(op);
                    }
                }
            }
        }
    }

    pub fn redo(&mut self) {
        // To be implemented symmetrically
    }

    pub fn clear_preview(&mut self) {
        self.preview = None;
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
}
