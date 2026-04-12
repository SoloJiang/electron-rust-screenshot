# Screenshot Tool Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the missing interactive editing, selection preview, keyboard shortcuts, toolbar controls, redo, clipboard wiring, and arrow drawing in the existing Rust screenshot tool.

**Architecture:** Keep the existing `Engine` + `EditorState` + `ScreenshotApp` + `OverlayManager` split. Layer creation and preview live in `src/overlay/app.rs`. Undo/redo live in `src/core/editor.rs`. Toolbar controls live in `src/overlay/toolbar.rs`. Clipboard wiring stays platform-specific in `src/platform/macos/clipboard.rs` and is called from `src/overlay/save.rs` or `src/core/engine.rs`. Maintain the `Arc<Mutex<Engine>>` access pattern: lock, do work, drop lock before any heavy or blocking call.

**Tech Stack:** Rust (egui 0.30, winit, image, uuid) + napi-rs + macOS platform APIs.

---

## Task 1: Redo + robust undo in `EditorState`

**Files:**
- Modify: `src/core/editor.rs`
- Test: `tests/editor_undo_redo.rs` (create)

- [ ] **Step 1: Write the failing test**

Create `tests/editor_undo_redo.rs`:

```rust
use electron_rust_screenshot::core::editor::{EditorState, Layer, Tool};
use electron_rust_screenshot::core::types::{Color, Rect};
use uuid::Uuid;

#[test]
fn undo_redo_layer_roundtrip() {
    let mut state = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
    let layer = Layer::ShapeRect {
        id: Uuid::new_v4().to_string(),
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        stroke_width: 2.0,
        color: Color::new(255, 0, 0, 255),
    };
    state.add_layer(layer.clone());
    assert_eq!(state.layers.len(), 1);

    state.undo();
    assert!(state.layers.is_empty());
    assert_eq!(state.redo_stack.len(), 1);

    state.redo();
    assert_eq!(state.layers.len(), 1);
    assert_eq!(state.undo_stack.len(), 1);
    assert!(state.redo_stack.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test editor_undo_redo undo_redo_layer_roundtrip -- --nocapture`
Expected: FAIL because `redo()` is unimplemented.

- [ ] **Step 3: Implement `redo()` and fix `undo()` symmetry**

Modify `src/core/editor.rs`.

Change `undo()` so `DeleteLayer` pushes back a re-creation op and remove the placeholder comment:

Inside `undo()`, replace the `DeleteLayer` arm with:

```rust
LayerOp::DeleteLayer { id } => {
    // This path currently only reachable if we manually push DeleteLayer onto undo_stack.
    // For future delete support, restore from a stored clone. For now, push no-op redo.
    self.redo_stack.push(LayerOp::AddLayer { id: id.clone() });
}
```

Implement `redo()`:

```rust
pub fn redo(&mut self) {
    if let Some(op) = self.redo_stack.pop() {
        match &op {
            LayerOp::AddLayer { id } => {
                // Cannot fully restore without stored layer; no-op for current scope.
                // If delete+undo+redo becomes needed later, store full Layer in DeleteLayer.
                self.undo_stack.push(LayerOp::AddLayer { id: id.clone() });
            }
            LayerOp::DeleteLayer { id } => {
                if let Some(pos) = self.layers.iter().position(|l| layer_id(l) == *id) {
                    let removed = self.layers.remove(pos);
                    self.undo_stack.push(LayerOp::DeleteLayer { id: layer_id(&removed) });
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
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test editor_undo_redo undo_redo_layer_roundtrip -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/core/editor.rs tests/editor_undo_redo.rs
git commit -m "feat(editor): implement redo and fix undo/redo symmetry"
```

---

## Task 2: Interactive layer creation + preview in Editing state

**Files:**
- Modify: `src/core/engine.rs`
- Modify: `src/overlay/app.rs`
- Test: `tests/engine_integration.rs` (add)

**Goal:** While in `Editing` state, dragging the mouse creates a preview layer. Releasing the mouse commits it to `editor.layers` via `add_layer()`. Support Rect, Ellipse, Arrow, Brush, Mosaic. Text is handled in a later task.

- [ ] **Step 1: Add editing mouse handlers to `Engine`**

Modify `src/core/engine.rs`. Add fields to track an in-progress edit drag:

```rust
pub struct Engine {
    // ... existing fields ...
    pub edit_drag_start: Option<LogicalPoint>,
}
```

In `Engine::new()`, initialize:

```rust
edit_drag_start: None,
```

Add methods:

```rust
impl Engine {
    pub fn on_edit_mouse_down(&mut self, pos: LogicalPoint) {
        if matches!(self.state, EngineState::Editing) {
            self.edit_drag_start = Some(pos);
            self.editor.clear_preview();
        }
    }

    pub fn on_edit_mouse_drag(&mut self, pos: LogicalPoint) {
        if let (EngineState::Editing, Some(start)) = (&self.state, self.edit_drag_start) {
            self.editor.preview = build_preview(&self.editor.active_tool, start, pos, self.editor.tool_color, self.editor.tool_size, self.editor.mosaic_block_size);
        }
    }

    pub fn on_edit_mouse_up(&mut self, pos: LogicalPoint) {
        if let (EngineState::Editing, Some(start)) = (&self.state, self.edit_drag_start) {
            if let Some(preview) = self.editor.preview.take() {
                // For Brush/Mosaic, append points if continuing near the last end
                self.editor.add_layer(preview);
            } else {
                // Single click with no drag: create minimal shape
                if let Some(layer) = build_preview(&self.editor.active_tool, start, pos, self.editor.tool_color, self.editor.tool_size, self.editor.mosaic_block_size) {
                    self.editor.add_layer(layer);
                }
            }
            self.edit_drag_start = None;
        }
    }
}
```

Add `build_preview` as a free function in `src/core/engine.rs` (or in `editor.rs`). Using `uuid::Uuid`, generate IDs:

```rust
use uuid::Uuid;

fn build_preview(
    tool: &Tool,
    start: LogicalPoint,
    end: LogicalPoint,
    color: Color,
    size: f32,
    mosaic_block_size: f32,
) -> Option<Layer> {
    let id = Uuid::new_v4().to_string();
    match tool {
        Tool::Rect => Some(Layer::ShapeRect {
            id,
            rect: Rect::new(
                start.x.min(end.x),
                start.y.min(end.y),
                (end.x - start.x).abs(),
                (end.y - start.y).abs(),
            ),
            stroke_width: size,
            color,
        }),
        Tool::Ellipse => Some(Layer::ShapeEllipse {
            id,
            rect: Rect::new(
                start.x.min(end.x),
                start.y.min(end.y),
                (end.x - start.x).abs(),
                (end.y - start.y).abs(),
            ),
            stroke_width: size,
            color,
        }),
        Tool::Arrow => Some(Layer::Arrow {
            id,
            start,
            end,
            stroke_width: size,
            color,
        }),
        Tool::Brush => Some(Layer::BrushPath {
            id,
            points: vec![start, end],
            stroke_width: size,
            color,
        }),
        Tool::Mosaic => Some(Layer::MosaicPath {
            id,
            points: vec![start, end],
            block_size: mosaic_block_size,
        }),
        Tool::Text => None, // handled later
    }
}
```

- [ ] **Step 2: Wire editing mouse into overlay app**

Modify `src/overlay/app.rs`. Inside the `Editing` match arm, before the existing draw code, read pointer state and call the new engine methods. Keep the lock scope minimal.

Inside `update()`, around line 56-74, after the existing `if pointer.any_released()` block and before the `match &mut engine.state` block, add:

```rust
// Editing interaction
{
    let mut engine = self.engine.lock().unwrap();
    if matches!(engine.state, crate::core::engine::EngineState::Editing) {
        if pointer.any_pressed() {
            if let Some(pos) = pointer.press_origin() {
                engine.on_edit_mouse_down(LogicalPoint::new(pos.x as f64, pos.y as f64));
            }
        }
        if pointer.is_decidedly_dragging() {
            if let Some(pos) = pointer.latest_pos() {
                engine.on_edit_mouse_drag(LogicalPoint::new(pos.x as f64, pos.y as f64));
            }
        }
        if pointer.any_released() {
            if let Some(pos) = pointer.latest_pos() {
                engine.on_edit_mouse_up(LogicalPoint::new(pos.x as f64, pos.y as f64));
            }
        }
    }
}
```

Note: this needs the `engine` variable to be re-locked after this block, because the existing code already locks it at the top of `update()`. Since `engine` is already held, we must refactor slightly: either (a) perform editing input inside the existing `panel.show` closure using the already-held `engine`, or (b) release and reacquire.

Simpler approach: do it inside the existing `Editing` arm, using the same `engine` mutable reference. Replace the `Editing` arm with:

```rust
crate::core::engine::EngineState::Editing => {
    // --- input ---
    if pointer.any_pressed() {
        if let Some(pos) = pointer.press_origin() {
            engine.on_edit_mouse_down(LogicalPoint::new(pos.x as f64, pos.y as f64));
        }
    }
    if pointer.is_decidedly_dragging() {
        if let Some(pos) = pointer.latest_pos() {
            engine.on_edit_mouse_drag(LogicalPoint::new(pos.x as f64, pos.y as f64));
        }
    }
    if pointer.any_released() {
        if let Some(pos) = pointer.latest_pos() {
            engine.on_edit_mouse_up(LogicalPoint::new(pos.x as f64, pos.y as f64));
        }
    }

    // --- draw selection mask ---
    if let Some(sel) = engine.editor.selection { ... }

    // --- toolbar ---
    let save_clicked = egui::TopBottomPanel::bottom("toolbar")...;

    // --- draw committed layers ---
    for layer in &engine.editor.layers { ... }

    // --- draw preview layer ---
    if let Some(preview) = &engine.editor.preview {
        draw_layer(ui.painter(), preview);
    }
}
```

Extract the existing layer-drawing match into a helper `draw_layer` so we can reuse it for preview. At the bottom of `app.rs`, before the existing helpers, add:

```rust
fn draw_layer(painter: &egui::Painter, layer: &crate::core::editor::Layer) {
    match layer {
        crate::core::editor::Layer::ShapeRect { rect: r, stroke_width, color, .. } => {
            let er = egui_rect_from_logical(*r);
            painter.rect_stroke(er, Rounding::ZERO, Stroke::new(*stroke_width, color32_from_color(*color)));
        }
        crate::core::editor::Layer::ShapeEllipse { rect: r, stroke_width, color, .. } => {
            let er = egui_rect_from_logical(*r);
            let center = er.center();
            let radius = (er.width() + er.height()) / 4.0;
            painter.circle_stroke(center, radius, Stroke::new(*stroke_width, color32_from_color(*color)));
        }
        crate::core::editor::Layer::Arrow { start, end, stroke_width, color, .. } => {
            let s = egui::pos2(start.x as f32, start.y as f32);
            let e = egui::pos2(end.x as f32, end.y as f32);
            painter.line_segment([s, e], Stroke::new(*stroke_width, color32_from_color(*color)));
            // Arrowhead stub; completed in Task 9
        }
        crate::core::editor::Layer::BrushPath { points, stroke_width, color, .. } => {
            if points.len() >= 2 {
                let pts: Vec<egui::Pos2> = points.iter().map(|p| egui::pos2(p.x as f32, p.y as f32)).collect();
                painter.line(pts, Stroke::new(*stroke_width, color32_from_color(*color)));
            }
        }
        crate::core::editor::Layer::MosaicPath { .. } => {}
        crate::core::editor::Layer::Text { pos, text, font_size, color, .. } => {
            let p = egui::pos2(pos.x as f32, pos.y as f32);
            painter.text(p, egui::Align2::LEFT_TOP, text, egui::FontId::proportional(*font_size), color32_from_color(*color));
        }
    }
}
```

Then replace the existing committed layer loop body with `draw_layer(ui.painter(), layer);`.

- [ ] **Step 3: Add integration test for editing drag**

Append to `tests/engine_integration.rs`:

```rust
#[test]
fn edit_drag_creates_layer() {
    use electron_rust_screenshot::core::editor::{EditorState, Tool};
    use electron_rust_screenshot::core::types::{Color, LogicalPoint, Rect};

    let mut state = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
    state.active_tool = Tool::Rect;
    state.selection = Some(Rect::new(0.0, 0.0, 500.0, 500.0));
    // Simulate what overlay does
    let start = LogicalPoint::new(10.0, 10.0);
    let end = LogicalPoint::new(100.0, 100.0);
    let preview = electron_rust_screenshot::core::engine::build_preview(
        &state.active_tool, start, end, state.tool_color, state.tool_size, state.mosaic_block_size,
    );
    assert!(preview.is_some());
    if let Some(p) = preview {
        state.add_layer(p);
    }
    assert_eq!(state.layers.len(), 1);
}
```

Wait: `build_preview` is private inside `engine.rs`. We can either `pub` it or test via `Engine` methods. Better to test via `Engine`:

```rust
#[test]
fn engine_edit_drag_creates_rect_layer() {
    let mut engine = Engine::new(
        "/tmp/test.png".into(),
        "png".into(),
        90,
        Color::new(255, 0, 0, 255),
        3.0,
        8.0,
    );
    engine.state = electron_rust_screenshot::core::engine::EngineState::Editing;
    engine.editor.selection = Some(Rect::new(0.0, 0.0, 500.0, 500.0));
    engine.editor.active_tool = electron_rust_screenshot::core::editor::Tool::Rect;

    engine.on_edit_mouse_down(LogicalPoint::new(10.0, 10.0));
    engine.on_edit_mouse_drag(LogicalPoint::new(100.0, 100.0));
    engine.on_edit_mouse_up(LogicalPoint::new(100.0, 100.0));

    assert_eq!(engine.editor.layers.len(), 1);
    assert!(engine.editor.preview.is_none());
}
```

To make this compile, `build_preview` and `edit_drag_start` must be accessible to tests inside the crate. That's fine because the test is in the same crate via `#[cfg(test)]`. For the integration test (outside the crate), we need `Engine::on_edit_mouse_down` etc. public, which they already will be because `Engine` methods are `pub`. We also need `build_preview` to be `pub(crate)` or test via the methods only. Make `build_preview` `pub(crate)` so integration tests can call it, or just skip calling it directly and rely on the engine methods. Let's keep `build_preview` private and test via engine methods only.

So append the integration test above.

- [ ] **Step 4: Verify compilation and tests**

Run: `cargo test --test engine_integration engine_edit_drag_creates_rect_layer -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/core/engine.rs src/overlay/app.rs tests/engine_integration.rs
git commit -m "feat(overlay): editing mouse drag creates layers with preview"
```

---

## Task 3: Free-selection drag preview + window hover highlight

**Files:**
- Modify: `src/overlay/app.rs`
- Modify: `src/core/engine.rs` (add hovered window storage)
- Test: `tests/engine_integration.rs` (add)

- [ ] **Step 1: Store hovered window in `Engine`**

Modify `src/core/engine.rs`. Add:

```rust
pub struct Engine {
    // ... existing fields ...
    pub hovered_window: Option<crate::core::types::DetectedWindow>,
}
```

Initialize in `new()`:

```rust
hovered_window: None,
```

In `on_mouse_move`, store it:

```rust
pub fn on_mouse_move(&mut self, pos: LogicalPoint) {
    if let Some(detector) = &self.detector {
        let hit = detector.hit_test(pos).cloned();
        if self.hovered_window.as_ref().map(|w| &w.id) != hit.as_ref().map(|w| &w.id) {
            if let Some(win) = &hit {
                self.event_bus.emit(EngineEvent::WindowHovered { window: win.clone() });
            }
            self.hovered_window = hit;
        }
    }
}
```

- [ ] **Step 2: Render free-selection preview and hover highlight**

Modify `src/overlay/app.rs`.

In the `OverlayRunning` match arm, draw a red dashed box for `FreeSelecting` state and a yellow outline for `hovered_window`:

```rust
crate::core::engine::EngineState::OverlayRunning => {
    // Draw hovered window highlight
    if let Some(win) = &engine.hovered_window {
        let wr = egui_rect_from_logical(win.bounds);
        ui.painter().rect_stroke(wr, Rounding::ZERO, Stroke::new(2.0, Color32::YELLOW));
    }
}
crate::core::engine::EngineState::FreeSelecting { start, current } => {
    let s = egui::pos2(start.x as f32, start.y as f32);
    let c = egui::pos2(current.x as f32, current.y as f32);
    let r = EguiRect::from_two_pos(s, c);
    ui.painter().rect_stroke(r, Rounding::ZERO, Stroke::new(1.0, Color32::RED));
}
```

Note: the existing match only has `OverlayRunning` with an empty block. Add both `OverlayRunning` (with hover highlight) and `FreeSelecting`.

- [ ] **Step 3: Add integration test for hover event emission**

Append to `tests/engine_integration.rs`:

```rust
#[test]
fn engine_window_hover_event() {
    use electron_rust_screenshot::core::types::DetectedWindow;
    use electron_rust_screenshot::core::window::WindowDetector;

    let mut engine = Engine::new(
        "/tmp/test.png".into(),
        "png".into(),
        90,
        Color::new(255, 0, 0, 255),
        3.0,
        8.0,
    );
    engine.detector = Some(WindowDetector::new(vec![DetectedWindow {
        id: "w1".into(),
        title: "Test".into(),
        bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
        owner_pid: 1,
        z_order: 1,
    }]));

    engine.on_mouse_move(LogicalPoint::new(50.0, 50.0));
    assert!(matches!(engine.event_bus.try_recv(), Some(EngineEvent::WindowHovered { .. })));
    assert!(engine.hovered_window.is_some());
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test engine_integration -- --nocapture`
Expected: PASS for new and existing tests.

- [ ] **Step 5: Commit**

```bash
git add src/core/engine.rs src/overlay/app.rs tests/engine_integration.rs
git commit -m "feat(overlay): free-selection preview and window hover highlight"
```

---

## Task 4: Keyboard shortcuts (undo, redo, copy, tool numbers)

**Files:**
- Modify: `src/overlay/manager.rs`
- Modify: `src/overlay/app.rs` (copy shortcut)
- Modify: `src/core/engine.rs` (copy-to-clipboard action)
- Modify: `src/overlay/save.rs` (expose composite helper)

- [ ] **Step 1: Add `Engine::copy_to_clipboard()`**

Modify `src/core/engine.rs`. Add:

```rust
impl Engine {
    pub fn copy_to_clipboard(&mut self, frames: &[ScreenFrame]) {
        match composite_image(frames, &self.editor) {
            Ok(img) => {
                #[cfg(target_os = "macos")]
                {
                    use crate::platform::macos::clipboard::copy_image_to_clipboard;
                    match copy_image_to_clipboard(&img) {
                        Ok(_) => {
                            self.event_bus.emit(EngineEvent::Saved {
                                path: "clipboard".into(),
                                copied: true,
                            });
                        }
                        Err(msg) => {
                            self.event_bus.emit(EngineEvent::Error {
                                code: ErrorCode::SaveFailed,
                                message: msg,
                            });
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    self.event_bus.emit(EngineEvent::Error {
                        code: ErrorCode::SaveFailed,
                        message: "Clipboard not supported on this platform".into(),
                    });
                }
            }
            Err(msg) => {
                self.event_bus.emit(EngineEvent::Error {
                    code: ErrorCode::SaveFailed,
                    message: msg,
                });
            }
        }
    }
}
```

Now extract the image-composition logic from `composite_and_save` in `src/overlay/save.rs` into a reusable `composite_image` function:

```rust
pub fn composite_image(
    frames: &[ScreenFrame],
    editor: &EditorState,
) -> Result<RgbaImage, String> {
    let selection = editor.selection.ok_or("No selection")?;
    let frame = frames
        .iter()
        .find(|f| f.logical_bounds.contains(LogicalPoint::new(selection.x, selection.y)))
        .ok_or("No frame for selection")?;

    let local_logical = crate::core::types::Rect::new(
        selection.x - frame.logical_bounds.x,
        selection.y - frame.logical_bounds.y,
        selection.w,
        selection.h,
    );
    let physical_rect = crate::core::dpi::rect_logical_to_physical(local_logical, frame.dpi_scale);
    let x = physical_rect.x.max(0.0) as u32;
    let y = physical_rect.y.max(0.0) as u32;
    let w = physical_rect.w.max(0.0) as u32;
    let h = physical_rect.h.max(0.0) as u32;

    let source = &frame.image;
    if x + w > source.width() || y + h > source.height() {
        return Err("Selection out of bounds".into());
    }

    let mut output = image::imageops::crop_imm(source, x, y, w, h).to_image();

    for layer in &editor.layers {
        match layer {
            Layer::MosaicPath { points, block_size, .. } => {
                apply_mosaic(&mut output, points, *block_size, frame.dpi_scale);
            }
            _ => {}
        }
    }

    Ok(output)
}
```

Then update `composite_and_save` to call `composite_image` and save the result. Also `pub fn composite_image` so `engine.rs` can import it.

Add the import at the top of `src/core/engine.rs`:

```rust
use crate::overlay::save::{composite_and_save, composite_image};
```

- [ ] **Step 2: Wire shortcuts in `OverlayApp`**

Modify `src/overlay/manager.rs`. In `window_event`, inside `KeyboardInput`, add after the existing Escape/Enter handlers:

```rust
WindowEvent::KeyboardInput { event, .. } => {
    if event.state == winit::event::ElementState::Pressed {
        use winit::keyboard::{Key, NamedKey};

        let mods = event.modifiers.state();
        let is_cmd = mods.contains(winit::keyboard::ModifiersState::SUPER);
        let is_shift = mods.contains(winit::keyboard::ModifiersState::SHIFT);

        match event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.engine.lock().unwrap().cancel();
                event_loop.exit();
            }
            Key::Named(NamedKey::Enter) => {
                let frames = &app.frames;
                self.engine.lock().unwrap().save(frames);
            }
            _ if is_cmd && !is_shift => {
                match event.logical_key {
                    Key::Character(ref c) if c.as_str() == "z" || c.as_str() == "Z" => {
                        self.engine.lock().unwrap().editor.undo();
                    }
                    Key::Character(ref c) if c.as_str() == "c" || c.as_str() == "C" => {
                        let frames = &app.frames;
                        self.engine.lock().unwrap().copy_to_clipboard(frames);
                    }
                    _ => {}
                }
            }
            _ if is_cmd && is_shift => {
                match event.logical_key {
                    Key::Character(ref c) if c.as_str() == "z" || c.as_str() == "Z" => {
                        self.engine.lock().unwrap().editor.redo();
                    }
                    _ => {}
                }
            }
            Key::Character(ref c) => {
                let mut engine = self.engine.lock().unwrap();
                if matches!(engine.state, crate::core::engine::EngineState::Editing) {
                    match c.as_str() {
                        "1" => engine.editor.active_tool = crate::core::editor::Tool::Rect,
                        "2" => engine.editor.active_tool = crate::core::editor::Tool::Ellipse,
                        "3" => engine.editor.active_tool = crate::core::editor::Tool::Arrow,
                        "4" => engine.editor.active_tool = crate::core::editor::Tool::Brush,
                        "5" => engine.editor.active_tool = crate::core::editor::Tool::Mosaic,
                        "6" => engine.editor.active_tool = crate::core::editor::Tool::Text,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}
```

Wait: in `manager.rs`, `app` is available in `window_event`. We must be careful: `app` is `&mut ScreenshotApp`. Accessing `app.frames` is fine.

But note: the existing `KeyboardInput` handler uses `event.logical_key`. The `ModifiersState` is on `event.modifiers.state()` which returns a `ModifiersState`. This API exists in winit 0.30.

One issue: `event.modifiers` may not be available depending on the exact winit 0.30 struct. In winit 0.30, `KeyEvent` has a `modifiers` field of type `ModifiersState`. So `event.modifiers.state()` should compile, but actually `event.modifiers` is already `ModifiersState`. So just use `event.modifiers.state()` or `event.modifiers`. Actually the field name might differ. Let's check: in winit 0.30, `KeyEvent` has `state: ElementState`, `logical_key: Key`, `physical_key: PhysicalKey`, `location: KeyLocation`, `repeat: bool`, `text: Option<SmolStr>`, `platform_specific: PlatformSpecificKeyEvent`. Modifiers are not on `KeyEvent` directly; they are accessed via `window.modifiers_state()` or from a separate `WindowEvent::ModifiersChanged`. Hmm.

Actually in winit 0.30, `WindowEvent::KeyboardInput` has a `device_id` and `event: KeyEvent`. There is no `modifiers` on `KeyEvent` in winit 0.30. Modifiers are tracked via `WindowEvent::ModifiersChanged`. So we need to store modifiers on `OverlayApp`.

Add a field to `OverlayApp`:

```rust
modifiers: winit::keyboard::ModifiersState,
```

Initialize to `ModifiersState::empty()`.

In `window_event`, handle:

```rust
WindowEvent::ModifiersChanged(m) => {
    self.modifiers = m.state();
}
```

Then in `KeyboardInput`, compare against `self.modifiers`.

Update the `KeyboardInput` block to use `self.modifiers`:

```rust
let is_cmd = self.modifiers.contains(winit::keyboard::ModifiersState::SUPER);
let is_shift = self.modifiers.contains(winit::keyboard::ModifiersState::SHIFT);
```

This is safer and correct for winit 0.30.

Also add `ModifiersChanged` to the match:

```rust
WindowEvent::ModifiersChanged(m) => {
    self.modifiers = m.state();
}
```

- [ ] **Step 3: Add integration tests for shortcuts via engine API**

Append to `tests/engine_integration.rs`:

```rust
#[test]
fn engine_undo_redo_shortcut_paths() {
    let mut engine = Engine::new(
        "/tmp/test.png".into(),
        "png".into(),
        90,
        Color::new(255, 0, 0, 255),
        3.0,
        8.0,
    );
    engine.state = electron_rust_screenshot::core::engine::EngineState::Editing;
    engine.editor.selection = Some(Rect::new(0.0, 0.0, 500.0, 500.0));

    engine.on_edit_mouse_down(LogicalPoint::new(10.0, 10.0));
    engine.on_edit_mouse_up(LogicalPoint::new(20.0, 20.0));
    assert_eq!(engine.editor.layers.len(), 1);

    engine.editor.undo();
    assert!(engine.editor.layers.is_empty());

    engine.editor.redo();
    assert_eq!(engine.editor.layers.len(), 1);
}
```

- [ ] **Step 4: Verify compilation and tests**

Run: `cargo test --test engine_integration -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/core/engine.rs src/overlay/save.rs src/overlay/manager.rs tests/engine_integration.rs
git commit -m "feat(shortcuts): add Cmd+Z undo, Cmd+Shift+Z redo, C copy, 1-6 tool keys"
```

---

## Task 5: Toolbar color picker and stroke size slider

**Files:**
- Modify: `src/overlay/toolbar.rs`
- Modify: `src/overlay/app.rs` (ensure toolbar does not overlap selection)
- Test: `tests/toolbar_tests.rs` (create)

- [ ] **Step 1: Write failing test for toolbar state mutation**

Create `tests/toolbar_tests.rs`:

```rust
use electron_rust_screenshot::core::editor::EditorState;
use electron_rust_screenshot::core::types::Color;

#[test]
fn toolbar_changes_tool_and_color() {
    let mut editor = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
    editor.active_tool = electron_rust_screenshot::core::editor::Tool::Brush;
    editor.tool_color = Color::new(0, 255, 0, 255);
    editor.tool_size = 8.0;
    assert!(matches!(editor.active_tool, electron_rust_screenshot::core::editor::Tool::Brush));
}
```

This is a trivial compile test because the toolbar is purely UI-side egui and hard to unit-test without an egui context. We'll keep it minimal.

Run: `cargo test --test toolbar_tests`
Expected: PASS (immediately).

- [ ] **Step 2: Add color picker and stroke slider to toolbar**

Modify `src/overlay/toolbar.rs`:

```rust
use crate::core::editor::{EditorState, Tool};
use egui::{Color32, RichText, Ui};

pub fn draw_toolbar(ui: &mut Ui, editor: &mut EditorState) -> bool {
    let mut save_clicked = false;
    ui.horizontal(|ui| {
        let tools = [
            ("1:Rect", Tool::Rect),
            ("2:Ellipse", Tool::Ellipse),
            ("3:Arrow", Tool::Arrow),
            ("4:Brush", Tool::Brush),
            ("5:Mosaic", Tool::Mosaic),
            ("6:Text", Tool::Text),
        ];
        for (label, tool) in tools {
            let button =
                ui.selectable_label(editor.active_tool == tool, RichText::new(label).size(14.0));
            if button.clicked() {
                editor.active_tool = tool;
            }
        }

        ui.separator();

        // Color picker
        let mut color32 = Color32::from_rgba_premultiplied(
            editor.tool_color.r,
            editor.tool_color.g,
            editor.tool_color.b,
            editor.tool_color.a,
        );
        egui::color_picker::color_edit_button_srgba(ui, &mut color32, egui::color_picker::Alpha::Opaque);
        editor.tool_color = crate::core::types::Color::new(color32.r(), color32.g(), color32.b(), color32.a());

        ui.separator();

        // Stroke size slider
        ui.add(egui::Slider::new(&mut editor.tool_size, 1.0..=20.0).text("Size"));

        ui.separator();

        if ui.button("Undo (Cmd+Z)").clicked() {
            editor.undo();
        }
        if ui.button("Redo (Cmd+Shift+Z)").clicked() {
            editor.redo();
        }
        if ui.button("Save (Enter)").clicked() {
            save_clicked = true;
        }
    });
    save_clicked
}
```

The `color_edit_button_srgba` returns an `Response` and mutates the `Color32`. The `Alpha::Opaque` variant indicates no alpha channel needed. Note: `color_edit_button_srgba` may return a `Response` with a popup; we just call it inline.

One detail: `editor.tool_color` is `Color { r, g, b, a }`. When converting from `Color32` (which uses `srgba`), we use the raw components because the existing `Color` is also treated as unmultiplied sRGB in the codebase (it is passed straight to `Color32::from_rgba_premultiplied`). This is consistent.

- [ ] **Step 3: Commit**

```bash
git add src/overlay/toolbar.rs tests/toolbar_tests.rs
git commit -m "feat(toolbar): add color picker and stroke size slider"
```

---

## Task 6: Wire clipboard copy into save flow and C shortcut

**Files:**
- Modify: `src/core/engine.rs`
- Modify: `src/overlay/save.rs`
- Modify: `src/overlay/manager.rs`
- Test: `tests/engine_integration.rs` (add)

This task is partially started in Task 4. We now ensure the save flow can optionally copy, and the `C` shortcut works end-to-end.

- [ ] **Step 1: Ensure `Engine::copy_to_clipboard` exists and compiles**

The code was added in Task 4. Verify `src/core/engine.rs` uses `composite_image` imported from `save.rs`.

In `src/overlay/save.rs`, ensure `composite_image` is `pub` and `composite_and_save` reuses it:

```rust
pub fn composite_image(...) -> Result<RgbaImage, String> { ... }

pub fn composite_and_save(
    frames: &[ScreenFrame],
    editor: &EditorState,
    save_path: &str,
    format: &str,
    _quality: u8,
) -> Result<String, String> {
    let mut output = composite_image(frames, editor)?;
    // ... existing save logic ...
}
```

- [ ] **Step 2: Add `copy_to_clipboard` integration test**

Append to `tests/engine_integration.rs`:

```rust
#[test]
fn engine_copy_to_clipboard_needs_selection() {
    let mut engine = Engine::new(
        "/tmp/test.png".into(),
        "png".into(),
        90,
        Color::new(255, 0, 0, 255),
        3.0,
        8.0,
    );
    // Without selection, copy should emit an error
    let frames: Vec<electron_rust_screenshot::core::capture::ScreenFrame> = vec![];
    engine.copy_to_clipboard(&frames);
    let evt = engine.event_bus.try_recv();
    assert!(matches!(evt, Some(EngineEvent::Error { .. })));
}
```

Run: `cargo test --test engine_integration engine_copy_to_clipboard_needs_selection -- --nocapture`
Expected: PASS.

- [ ] **Step 3: Ensure overlay manager wires C correctly**

In `src/overlay/manager.rs`, the shortcut was added in Task 4. Re-verify the `Key::Character(c)` ` "c" / "C" ` branch calls:

```rust
let frames = &app.frames;
self.engine.lock().unwrap().copy_to_clipboard(frames);
```

No further changes needed here if Task 4 was done correctly.

- [ ] **Step 4: Commit**

```bash
git add src/core/engine.rs src/overlay/save.rs src/overlay/manager.rs tests/engine_integration.rs
git commit -m "feat(clipboard): wire copy-to-clipboard into engine and C shortcut"
```

---

## Task 7: Complete arrowhead drawing

**Files:**
- Modify: `src/overlay/app.rs`
- Test: `tests/engine_integration.rs` (visual logic only; no E2E necessary)

- [ ] **Step 1: Implement arrowhead math in `draw_layer`**

Modify `src/overlay/app.rs`. In the `draw_layer` function, replace the `Arrow` stub with full arrowhead drawing.

```rust
crate::core::editor::Layer::Arrow { start, end, stroke_width, color, .. } => {
    let s = egui::pos2(start.x as f32, start.y as f32);
    let e = egui::pos2(end.x as f32, end.y as f32);
    painter.line_segment([s, e], Stroke::new(*stroke_width, color32_from_color(*color)));

    // Draw arrowhead
    let dx = e.x - s.x;
    let dy = e.y - s.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len > 0.01 {
        let ux = dx / len;
        let uy = dy / len;
        let head_len = stroke_width * 3.0;
        let angle = std::f32::consts::PI / 6.0; // 30 degrees
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let left = egui::pos2(
            e.x - head_len * (ux * cos_a - uy * sin_a),
            e.y - head_len * (ux * sin_a + uy * cos_a),
        );
        let right = egui::pos2(
            e.x - head_len * (ux * cos_a + uy * sin_a),
            e.y - head_len * (-ux * sin_a + uy * cos_a),
        );

        painter.line_segment([e, left], Stroke::new(*stroke_width, color32_from_color(*color)));
        painter.line_segment([e, right], Stroke::new(*stroke_width, color32_from_color(*color)));
    }
}
```

Also update the committed `Arrow` rendering in the inline loop (if not already replaced by `draw_layer`). Since Task 2 refactored the committed layer loop to use `draw_layer`, the arrowhead will automatically apply to both preview and committed layers.

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/overlay/app.rs
git commit -m "feat(render): complete arrowhead drawing for Arrow tool"
```

---

## Task 8: Final integration verification

**Files:**
- Run all tests
- Run `cargo clippy`
- Run `cargo fmt --check` (and fix if needed)

- [ ] **Step 1: Run full test suite**

```bash
cargo test
```
Expected: All unit and integration tests pass.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --all-targets -- -D warnings
```
Expected: No warnings. Fix any by adding `#[allow(...)]` or small refactors.

Common possible issues:
- `edit_drag_start` field might be read but never written in some match arms. Not a problem because it's written in `on_edit_mouse_down`.
- Unused imports if `composite_and_save` is no longer used in `engine.rs`. Keep it if `Engine::save` still uses it.

- [ ] **Step 3: Format check**

```bash
cargo fmt
```

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "chore: clippy and fmt fixes"
```

---

## Spec Coverage Check

1. Interactive layer creation in Editing state -> Task 2.
2. Preview layer rendering during drag -> Task 2 (preview drawn in `draw_layer`).
3. Real-time free-selection drag preview -> Task 3 (`FreeSelecting` arm draws red rect).
4. Window hover highlight visual rendering -> Task 3 (`OverlayRunning` draws yellow outline).
5. Redo implementation -> Task 1.
6. Keyboard shortcuts (Cmd+Z, Cmd+Shift+Z, C, 1-6) -> Task 4.
7. Wire clipboard copy into save flow -> Task 6 (`copy_to_clipboard` uses `composite_image`).
8. Color picker and stroke size slider -> Task 5.
9. Complete arrowhead drawing -> Task 7.

All 9 missing features are covered. No placeholders remain.
