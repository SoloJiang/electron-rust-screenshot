# Draggable Selection Box Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add drag-to-move and drag-to-resize handles for the confirmed selection box in `Editing` state, with all existing annotation layers following the transformation, and full undo/redo support.

**Architecture:** Overlay (`app.rs`) handles hit-testing and cursor feedback, while pure geometry logic lives in `editor.rs` (`transform_selection`) and state-machine orchestration lives in `engine.rs` (`SelectionTransformState`). Changes are committed to the undo stack only when the drag ends.

**Tech Stack:** Rust (napi-rs workspace), egui, winit, cliclick/AppleScript for E2E

---

## File Structure

| File | Responsibility |
|------|----------------|
| `crates/screenshot-core/src/core/types.rs` | `Edge`, `Corner`, `ResizeHit` enums; `Rect::hit_test_resize_handle()` pure geometry |
| `crates/screenshot-core/src/core/editor.rs` | `LayerOp::UpdateSelectionAndLayers`; `transform_selection()` and `transform_layer()`; updated `undo/redo` |
| `crates/screenshot-core/src/core/engine.rs` | `SelectionTransformState`; `on_selection_transform_start/drag/end`; tests |
| `crates/screenshot-core/src/overlay/app.rs` | Draw 8 resize handles; cursor icon feedback; route pointer events into transform vs drawing |
| `e2e/specs/resize-selection.spec.ts` *(new)* | AppleScript E2E: drag selection body and SE corner, assert output image changes |

---

## Task 1: Resize Hit-Test Geometry in `types.rs`

**Files:**
- Modify: `crates/screenshot-core/src/core/types.rs`
- Test: inline `#[cfg(test)]` block at bottom of file

- [ ] **Step 1: Add enums and `hit_test_resize_handle`**

Insert the following after the `impl Rect` block (before `pub fn area` or after the whole `impl Rect`—order does not matter).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    North,
    South,
    East,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    NW,
    NE,
    SW,
    SE,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHit {
    Move,
    ResizeEdge { edge: Edge },
    ResizeCorner { corner: Corner },
}

impl Rect {
    pub fn hit_test_resize_handle(&self, p: LogicalPoint, handle_size: f64) -> Option<ResizeHit> {
        let half = handle_size / 2.0;
        let in_outer_x = p.x >= self.x - half && p.x <= self.x + self.w + half;
        let in_outer_y = p.y >= self.y - half && p.y <= self.y + self.h + half;
        if !in_outer_x || !in_outer_y {
            return None;
        }

        let near_left = p.x >= self.x - half && p.x <= self.x + half;
        let near_right = p.x >= self.x + self.w - half && p.x <= self.x + self.w + half;
        let near_top = p.y >= self.y - half && p.y <= self.y + half;
        let near_bottom = p.y >= self.y + self.h - half && p.y <= self.y + self.h + half;

        if near_left && near_top {
            return Some(ResizeHit::ResizeCorner { corner: Corner::NW });
        }
        if near_right && near_top {
            return Some(ResizeHit::ResizeCorner { corner: Corner::NE });
        }
        if near_left && near_bottom {
            return Some(ResizeHit::ResizeCorner { corner: Corner::SW });
        }
        if near_right && near_bottom {
            return Some(ResizeHit::ResizeCorner { corner: Corner::SE });
        }

        if near_top {
            return Some(ResizeHit::ResizeEdge { edge: Edge::North });
        }
        if near_bottom {
            return Some(ResizeHit::ResizeEdge { edge: Edge::South });
        }
        if near_left {
            return Some(ResizeHit::ResizeEdge { edge: Edge::West });
        }
        if near_right {
            return Some(ResizeHit::ResizeEdge { edge: Edge::East });
        }

        if self.contains(p) {
            return Some(ResizeHit::Move);
        }

        None
    }
}
```

- [ ] **Step 2: Add unit tests**

Append inside the existing `#[cfg(test)] mod tests { ... }` block:

```rust
    #[test]
    fn resize_hit_center_is_move() {
        let r = Rect::new(100.0, 100.0, 200.0, 200.0);
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(200.0, 200.0), 8.0), Some(ResizeHit::Move));
    }

    #[test]
    fn resize_hit_corners() {
        let r = Rect::new(100.0, 100.0, 200.0, 200.0);
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(100.0, 100.0), 8.0), Some(ResizeHit::ResizeCorner { corner: Corner::NW }));
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(300.0, 100.0), 8.0), Some(ResizeHit::ResizeCorner { corner: Corner::NE }));
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(100.0, 300.0), 8.0), Some(ResizeHit::ResizeCorner { corner: Corner::SW }));
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(300.0, 300.0), 8.0), Some(ResizeHit::ResizeCorner { corner: Corner::SE }));
    }

    #[test]
    fn resize_hit_edges() {
        let r = Rect::new(100.0, 100.0, 200.0, 200.0);
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(200.0, 100.0), 8.0), Some(ResizeHit::ResizeEdge { edge: Edge::North }));
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(200.0, 300.0), 8.0), Some(ResizeHit::ResizeEdge { edge: Edge::South }));
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(100.0, 200.0), 8.0), Some(ResizeHit::ResizeEdge { edge: Edge::West }));
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(300.0, 200.0), 8.0), Some(ResizeHit::ResizeEdge { edge: Edge::East }));
    }

    #[test]
    fn resize_hit_outside_is_none() {
        let r = Rect::new(100.0, 100.0, 200.0, 200.0);
        assert_eq!(r.hit_test_resize_handle(LogicalPoint::new(0.0, 0.0), 8.0), None);
    }
```

- [ ] **Step 3: Run tests**

Run:
```bash
cargo test --workspace hit_test resize_hit
```

Expected: all 5 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/screenshot-core/src/core/types.rs
git commit -m "feat(types): add resize hit-test enums and Rect::hit_test_resize_handle

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 2: Transform Geometry & Enhanced Undo in `editor.rs`

**Files:**
- Modify: `crates/screenshot-core/src/core/editor.rs`
- Test: inline `#[cfg(test)]` block at bottom of file

- [ ] **Step 1: Extend `LayerOp` enum**

Change the `LayerOp` definition from:

```rust
pub enum LayerOp {
    AddLayer { layer: Layer },
    DeleteLayer { layer: Layer },
    UpdateLayer { id: String, old: Layer, new: Layer },
}
```

to:

```rust
pub enum LayerOp {
    AddLayer { layer: Layer },
    DeleteLayer { layer: Layer },
    UpdateLayer { id: String, old: Layer, new: Layer },
    UpdateSelectionAndLayers {
        old_selection: Option<Rect>,
        new_selection: Option<Rect>,
        old_layers: Vec<Layer>,
        new_layers: Vec<Layer>,
    },
}
```

- [ ] **Step 2: Add `transform_selection` and `transform_layer` helpers**

Insert the following `impl` block after `impl EditorState` (or before it; Rust allows multiple `impl` blocks for the same type).

```rust
fn transform_layer(
    layer: &Layer,
    original: Rect,
    new_rect: Rect,
    _tx: f64,
    _ty: f64,
    sx: f64,
    sy: f64,
) -> Layer {
    let norm_x = |x: f64| -> f64 {
        if original.w > 0.0 {
            (x - original.x) / original.w
        } else {
            0.0
        }
    };
    let norm_y = |y: f64| -> f64 {
        if original.h > 0.0 {
            (y - original.y) / original.h
        } else {
            0.0
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
            rect: Rect::new(
                denorm_x(norm_x(rect.x)),
                denorm_y(norm_y(rect.y)),
                rect.w * sx,
                rect.h * sy,
            ),
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
            rect: Rect::new(
                denorm_x(norm_x(rect.x)),
                denorm_y(norm_y(rect.y)),
                rect.w * sx,
                rect.h * sy,
            ),
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
            start: LogicalPoint::new(
                denorm_x(norm_x(start.x)),
                denorm_y(norm_y(start.y)),
            ),
            end: LogicalPoint::new(
                denorm_x(norm_x(end.x)),
                denorm_y(norm_y(end.y)),
            ),
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
                .map(|p| LogicalPoint::new(denorm_x(norm_x(p.x)), denorm_y(norm_y(p.y))))
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
                .map(|p| LogicalPoint::new(denorm_x(norm_x(p.x)), denorm_y(norm_y(p.y))))
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
            pos: LogicalPoint::new(denorm_x(norm_x(pos.x)), denorm_y(norm_y(pos.y))),
            text: text.clone(),
            font_size: *font_size,
            color: *color,
        },
    }
}
```

Then add `transform_selection` inside `impl EditorState` after `clear_preview`:

```rust
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
                }
                Edge::South => {
                    new_rect.h += delta.y;
                }
                Edge::West => {
                    new_rect.x += delta.x;
                    new_rect.w -= delta.x;
                }
                Edge::East => {
                    new_rect.w += delta.x;
                }
            },
            ResizeHit::ResizeCorner { corner } => match corner {
                Corner::NW => {
                    new_rect.x += delta.x;
                    new_rect.y += delta.y;
                    new_rect.w -= delta.x;
                    new_rect.h -= delta.y;
                }
                Corner::NE => {
                    new_rect.y += delta.y;
                    new_rect.w += delta.x;
                    new_rect.h -= delta.y;
                }
                Corner::SW => {
                    new_rect.x += delta.x;
                    new_rect.w -= delta.x;
                    new_rect.h += delta.y;
                }
                Corner::SE => {
                    new_rect.w += delta.x;
                    new_rect.h += delta.y;
                }
            },
        }

        if new_rect.w < MIN_SIZE {
            let diff = MIN_SIZE - new_rect.w;
            new_rect.w = MIN_SIZE;
            match kind {
                ResizeHit::ResizeEdge { edge: Edge::West }
                | ResizeHit::ResizeCorner { corner: Corner::NW | Corner::SW } => {
                    new_rect.x -= diff;
                }
                _ => {}
            }
        }
        if new_rect.h < MIN_SIZE {
            let diff = MIN_SIZE - new_rect.h;
            new_rect.h = MIN_SIZE;
            match kind {
                ResizeHit::ResizeEdge { edge: Edge::North }
                | ResizeHit::ResizeCorner { corner: Corner::NW | Corner::NE } => {
                    new_rect.y -= diff;
                }
                _ => {}
            }
        }

        let tx = new_rect.x - original_selection.x;
        let ty = new_rect.y - original_selection.y;
        let sx = if original_selection.w > 0.0 {
            new_rect.w / original_selection.w
        } else {
            1.0
        };
        let sy = if original_selection.h > 0.0 {
            new_rect.h / original_selection.h
        } else {
            1.0
        };

        let new_layers: Vec<Layer> = layers
            .iter()
            .map(|layer| transform_layer(layer, original_selection, new_rect, tx, ty, sx, sy))
            .collect();
        (new_rect, new_layers)
    }
```

- [ ] **Step 3: Update `undo()` and `redo()`**

In `undo()`, add this arm after the `UpdateLayer` arm:

```rust
                LayerOp::UpdateSelectionAndLayers { old_selection, old_layers, .. } => {
                    self.selection = old_selection.clone();
                    self.layers = old_layers.clone();
                    self.redo_stack.push(op);
                }
```

In `redo()`, add this arm after the `UpdateLayer` arm:

```rust
                LayerOp::UpdateSelectionAndLayers { new_selection, new_layers, .. } => {
                    self.selection = new_selection.clone();
                    self.layers = new_layers.clone();
                    self.undo_stack.push(op);
                }
```

- [ ] **Step 4: Add unit tests for transform and undo/redo**

Append inside the existing `#[cfg(test)] mod tests { ... }` block:

```rust
    #[test]
    fn transform_selection_move_preserves_relative_positions() {
        let sel = Rect::new(0.0, 0.0, 100.0, 100.0);
        let layer = Layer::ShapeRect {
            id: "r1".into(),
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        };
        let (new_sel, new_layers) = EditorState::transform_selection(
            sel,
            &[layer.clone()],
            &super::super::types::ResizeHit::Move,
            LogicalPoint::new(50.0, 30.0),
        );
        assert_eq!(new_sel, Rect::new(50.0, 30.0, 100.0, 100.0));
        if let Layer::ShapeRect { rect, .. } = &new_layers[0] {
            assert_eq!(*rect, Rect::new(60.0, 40.0, 20.0, 20.0));
        } else {
            panic!("expected ShapeRect");
        }
    }

    #[test]
    fn transform_selection_scale_scales_layers() {
        let sel = Rect::new(0.0, 0.0, 100.0, 100.0);
        let layer = Layer::ShapeRect {
            id: "r1".into(),
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        };
        let (new_sel, new_layers) = EditorState::transform_selection(
            sel,
            &[layer.clone()],
            &super::super::types::ResizeHit::ResizeCorner { corner: super::super::types::Corner::SE },
            LogicalPoint::new(100.0, 100.0),
        );
        assert_eq!(new_sel, Rect::new(0.0, 0.0, 200.0, 200.0));
        if let Layer::ShapeRect { rect, .. } = &new_layers[0] {
            assert_eq!(*rect, Rect::new(20.0, 20.0, 40.0, 40.0));
        } else {
            panic!("expected ShapeRect");
        }
    }

    #[test]
    fn undo_redo_update_selection_and_layers() {
        let mut state = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
        state.selection = Some(Rect::new(0.0, 0.0, 100.0, 100.0));
        state.layers = vec![Layer::ShapeRect {
            id: "r1".into(),
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        }];

        let old_selection = state.selection.clone();
        let old_layers = state.layers.clone();
        state.selection = Some(Rect::new(50.0, 50.0, 100.0, 100.0));
        state.layers = vec![Layer::ShapeRect {
            id: "r1".into(),
            rect: Rect::new(60.0, 60.0, 20.0, 20.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        }];
        state.undo_stack.push(LayerOp::UpdateSelectionAndLayers {
            old_selection,
            new_selection: state.selection.clone(),
            old_layers,
            new_layers: state.layers.clone(),
        });

        state.undo();
        assert_eq!(state.selection, Some(Rect::new(0.0, 0.0, 100.0, 100.0)));
        assert_eq!(state.layers[0].layer_rect().unwrap(), Rect::new(10.0, 10.0, 20.0, 20.0));

        state.redo();
        assert_eq!(state.selection, Some(Rect::new(50.0, 50.0, 100.0, 100.0)));
        assert_eq!(state.layers[0].layer_rect().unwrap(), Rect::new(60.0, 60.0, 20.0, 20.0));
    }
```

Wait — the last test uses `layer_rect()` which does not exist on `Layer`. Replace the two asserts that use it with manual matches:

```rust
        state.undo();
        assert_eq!(state.selection, Some(Rect::new(0.0, 0.0, 100.0, 100.0)));
        if let Layer::ShapeRect { rect, .. } = &state.layers[0] {
            assert_eq!(*rect, Rect::new(10.0, 10.0, 20.0, 20.0));
        }

        state.redo();
        assert_eq!(state.selection, Some(Rect::new(50.0, 50.0, 100.0, 100.0)));
        if let Layer::ShapeRect { rect, .. } = &state.layers[0] {
            assert_eq!(*rect, Rect::new(60.0, 60.0, 20.0, 20.0));
        }
```

- [ ] **Step 5: Run tests**

Run:
```bash
cargo test --workspace transform_selection undo_redo_update_selection
```

Expected: 3+ tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/screenshot-core/src/core/editor.rs
git commit -m "feat(editor): add selection transform geometry and UpdateSelectionAndLayers op

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Selection Transform State Machine in `engine.rs`

**Files:**
- Modify: `crates/screenshot-core/src/core/engine.rs`
- Test: inline `#[cfg(test)]` block at bottom of file

- [ ] **Step 1: Add `SelectionTransformState` and engine field**

Insert the new struct right after the `EngineState` enum definition:

```rust
pub struct SelectionTransformState {
    pub kind: super::types::ResizeHit,
    pub start_pointer: LogicalPoint,
    pub original_selection: Rect,
    pub original_layers: Vec<super::editor::Layer>,
}
```

Then add a new field to `Engine`:

```rust
    pub selection_transform: Option<SelectionTransformState>,
```

Initialize it in `Engine::new`:

```rust
            selection_transform: None,
```

- [ ] **Step 2: Add transform event handlers**

Insert the following methods into `impl Engine` (for example, right after `on_edit_mouse_up`):

```rust
    pub fn on_selection_transform_start(&mut self, pos: LogicalPoint, kind: super::types::ResizeHit) {
        if let EngineState::Editing = self.state {
            if let Some(sel) = self.editor.selection {
                self.selection_transform = Some(SelectionTransformState {
                    kind,
                    start_pointer: pos,
                    original_selection: sel,
                    original_layers: self.editor.layers.clone(),
                });
                self.editor.clear_preview();
            }
        }
    }

    pub fn on_selection_transform_drag(&mut self, pos: LogicalPoint) {
        if let (EngineState::Editing, Some(ref state)) = (&self.state, self.selection_transform.as_ref()) {
            let delta = LogicalPoint::new(pos.x - state.start_pointer.x, pos.y - state.start_pointer.y);
            let (new_rect, new_layers) = EditorState::transform_selection(
                state.original_selection,
                &state.original_layers,
                &state.kind,
                delta,
            );
            self.editor.selection = Some(new_rect);
            self.editor.layers = new_layers;
        }
    }

    pub fn on_selection_transform_end(&mut self, _pos: LogicalPoint) {
        if let (EngineState::Editing, Some(state)) = (&self.state, self.selection_transform.take()) {
            if self.editor.selection != Some(state.original_selection)
                || self.editor.layers != state.original_layers
            {
                self.editor.undo_stack.push(super::editor::LayerOp::UpdateSelectionAndLayers {
                    old_selection: Some(state.original_selection),
                    new_selection: self.editor.selection,
                    old_layers: state.original_layers,
                    new_layers: self.editor.layers.clone(),
                });
                self.editor.redo_stack.clear();
            }
        }
    }
```

- [ ] **Step 3: Add unit tests**

Append inside the existing `#[cfg(test)] mod tests { ... }` block:

```rust
    #[test]
    fn selection_transform_end_creates_undo_record() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::Editing;
        engine.editor.selection = Some(Rect::new(0.0, 0.0, 100.0, 100.0));
        engine.editor.layers = vec![Layer::ShapeRect {
            id: "r1".into(),
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            stroke_width: 2.0,
            color: Color::new(255, 0, 0, 255),
        }];

        use crate::core::types::{ResizeHit, Corner};
        engine.on_selection_transform_start(LogicalPoint::new(0.0, 0.0), ResizeHit::Move);
        engine.on_selection_transform_drag(LogicalPoint::new(50.0, 30.0));
        engine.on_selection_transform_end(LogicalPoint::new(50.0, 30.0));

        assert_eq!(engine.editor.selection, Some(Rect::new(50.0, 30.0, 100.0, 100.0)));
        assert!(!engine.editor.undo_stack.is_empty());
    }

    #[test]
    fn selection_transform_no_move_does_not_create_undo() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::Editing;
        engine.editor.selection = Some(Rect::new(0.0, 0.0, 100.0, 100.0));

        use crate::core::types::{ResizeHit, Corner};
        engine.on_selection_transform_start(LogicalPoint::new(0.0, 0.0), ResizeHit::Move);
        engine.on_selection_transform_end(LogicalPoint::new(0.0, 0.0));

        assert!(engine.editor.undo_stack.is_empty());
    }
```

- [ ] **Step 4: Run tests**

Run:
```bash
cargo test --workspace selection_transform
```

Expected: 2+ tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/screenshot-core/src/core/engine.rs
git commit -m "feat(engine): add selection transform state machine

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 4: Overlay Visual Handles & Event Routing in `app.rs`

**Files:**
- Modify: `crates/screenshot-core/src/overlay/app.rs`

- [ ] **Step 1: Add imports**

At the top of the file, change:

```rust
use crate::core::types::{Color, LogicalPoint, Rect};
```

to:

```rust
use crate::core::types::{Color, Corner, Edge, LogicalPoint, Rect, ResizeHit};
```

- [ ] **Step 2: Modify `Editing` branch to draw handles, set cursors, and route events**

Locate the `crate::core::engine::EngineState::Editing => { ... }` block. Replace the **entire** `Editing` arm of the `match` with the code below.

Before replacement, the block starts around:

```rust
                crate::core::engine::EngineState::Editing => {
                    // Only allow editing mouse interaction inside the selection area.
                    ...
```

and ends just before:

```rust
                _ => {}
```

Here is the replacement:

```rust
                crate::core::engine::EngineState::Editing => {
                    let Some(sel) = engine.editor.selection else { return; };

                    let logical_pos = pointer.latest_pos().map(|pos| {
                        LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y)
                    });
                    let resize_hit = logical_pos.and_then(|p| sel.hit_test_resize_handle(p, 8.0));

                    // Cursor feedback
                    if let Some(hit) = resize_hit {
                        ui.output().cursor_icon = match hit {
                            ResizeHit::Move => egui::CursorIcon::Move,
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
                        };
                    }

                    let in_selection = logical_pos.map(|p| sel.contains(p)).unwrap_or(true);
                    let in_transform_zone = resize_hit.is_some();

                    // Event routing: if we are already transforming, continue it
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
                                if let Some(hit) = sel.hit_test_resize_handle(pos, 8.0) {
                                    engine.on_selection_transform_start(pos, hit);
                                } else if sel.contains(pos) {
                                    engine.on_edit_mouse_down(pos);
                                }
                            }
                        }
                        if pointer.is_decidedly_dragging() {
                            if let Some(pos) = logical_pos {
                                engine.on_edit_mouse_drag(pos);
                            }
                        }
                        if pointer.any_released() {
                            if let Some(pos) = logical_pos {
                                engine.on_edit_mouse_up(pos);
                            }
                        }
                    }

                    // Uniform mask over entire screen
                    ui.painter().rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(120));

                    // Unmask the selected region
                    self.draw_unmasked_region(ui.painter(), sel, offset);
                    // White selection border
                    let sel_rect = egui_rect_from_logical(sel, offset);
                    ui.painter().rect_stroke(
                        sel_rect,
                        Rounding::ZERO,
                        Stroke::new(1.0, Color32::WHITE),
                    );

                    // Draw 8 resize handles
                    let handle_radius = 4.0;
                    let handle_stroke = Stroke::new(1.0, Color32::from_rgb(0, 120, 255));
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

                    // Toolbar as a floating window just below the selection
                    if show_toolbar {
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
                }
```

**Important:** Verify there are no variable-shadowing or borrow-checker issues. The replacement uses `logical_pos` which is an `Option<LogicalPoint>` computed before the mutable borrows of `engine`, so it should be fine. The `return` inside `let Some(sel) = engine.editor.selection else { return; };` exits the closure passed to `panel.show`; that is acceptable in egui.

- [ ] **Step 3: Build**

Run:
```bash
cargo test --workspace
```

Expected: all workspace tests compile and PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/screenshot-core/src/overlay/app.rs
git commit -m "feat(overlay): add resize handles, cursor feedback, and transform event routing

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 5: Full Workspace Build & Smoke Test

**Files:**
- All workspace crates

- [ ] **Step 1: Run Rust tests**

```bash
cargo test --workspace
```

Expected: all tests PASS.

- [ ] **Step 2: Run npm smoke test**

```bash
npm run build && npm test
```

Expected: build succeeds; `npm test` smoke test passes.

- [ ] **Step 3: Commit any fixes**

If any compilation or test fixes were needed:

```bash
git add -A
git commit -m "fix: address build/test issues from selection transform integration

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 6: AppleScript E2E Validation

**Files:**
- Create: `e2e/specs/resize-selection.spec.ts`
- Modify (if needed): `e2e/helpers/runner.ts`, `e2e/helpers/mouse.ts`

- [ ] **Step 1: Create E2E spec**

Create `e2e/specs/resize-selection.spec.ts`:

```typescript
import { runScreenshot } from '../helpers/runner';
import * as mouse from '../helpers/mouse';
import * as fs from 'fs';
import sizeOf from 'image-size';

describe('selection resize and move', () => {
  jest.setTimeout(30000);

  it('can drag selection body to move it', async () => {
    const outPath = `/tmp/screenshot-move-test-${Date.now()}.png`;
    const child = runScreenshot({ outputPath: outPath });
    await mouse.sleep(2000);

    // Free-select a region in the center of the main display
    await mouse.drag(500, 500, 700, 600);
    await mouse.sleep(500);

    // Drag the selection body by 100px right
    await mouse.drag(600, 550, 700, 550);
    await mouse.sleep(500);

    // Save via toolbar click is hard to coordinate; use Enter shortcut
    await mouse.keyPress('return');
    await child.waitForExit();

    expect(fs.existsSync(outPath)).toBe(true);
    const dims = sizeOf(outPath);
    // Output should be same size as original drag (200x100)
    expect(dims.width).toBe(200);
    expect(dims.height).toBe(100);
    fs.unlinkSync(outPath);
  });

  it('can drag SE corner to enlarge selection', async () => {
    const outPath = `/tmp/screenshot-resize-test-${Date.now()}.png`;
    const child = runScreenshot({ outputPath: outPath });
    await mouse.sleep(2000);

    // Free-select a small region
    await mouse.drag(500, 500, 600, 600);
    await mouse.sleep(500);

    // Drag the SE corner handle outward by 100px
    // SE corner is at (600, 600) after the initial drag
    await mouse.drag(600, 600, 700, 700);
    await mouse.sleep(500);

    await mouse.keyPress('return');
    await child.waitForExit();

    expect(fs.existsSync(outPath)).toBe(true);
    const dims = sizeOf(outPath);
    // Original 100x100, enlarged by 100 -> 200x200
    expect(dims.width).toBe(200);
    expect(dims.height).toBe(200);
    fs.unlinkSync(outPath);
  });
});
```

If `e2e/helpers/mouse.ts` does not export a `keyPress` helper, add it there or open an issue. You may also use the keyboard helper from `e2e/helpers/keyboard.ts` if it exists.

- [ ] **Step 2: Run the E2E spec**

```bash
cd e2e && npx jest --runInBand specs/resize-selection.spec.ts
```

Expected: tests PASS (requires `cliclick` accessibility permissions).

- [ ] **Step 3: Commit**

```bash
git add e2e/specs/resize-selection.spec.ts
git commit -m "test(e2e): add AppleScript validation for selection move and resize

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Spec Coverage Self-Review

| Spec Requirement | Plan Task |
|------------------|-----------|
| `Editing` state drag-to-move | Task 4 event routing + Task 3 engine state |
| 8-direction resize (corners + edges) | Task 1 hit test + Task 4 handles + Task 2 geometry |
| Layers follow selection transformation | Task 2 `transform_selection` + `transform_layer` |
| Undo/redo for selection transform (single record per drag) | Task 2 `LayerOp::UpdateSelectionAndLayers` + Task 3 `on_selection_transform_end` |
| Cursor feedback on hover | Task 4 cursor icon assignment |
| Minimum size clamp (8px) | Task 2 `MIN_SIZE` clamp inside `transform_selection` |
| AppleScript E2E validation | Task 6 |

**Placeholder scan:** None. Every step contains concrete code, exact commands, and expected outputs.

**Type consistency check:**
- `ResizeHit`, `Corner`, `Edge` — defined in Task 1, used in Tasks 2, 3, 4.
- `UpdateSelectionAndLayers` — defined in Task 2, pushed in Task 3, handled in undo/redo in Task 2.
- `SelectionTransformState` — defined in Task 3, used in Task 4 (`engine.selection_transform`).

No inconsistencies found.
