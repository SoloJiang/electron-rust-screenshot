# Multi-Monitor Support Design

## Goal
Enable the screenshot overlay to run natively across all connected displays on macOS.

## Requirements
- One overlay window per physical display.
- Support free-selection drag that starts on one display and ends on another.
- Once a selection is confirmed (Enter or toolbar Save), the user enters Editing state on the **primary screen of the selection**; all other windows become non-interactive (display-only).
- Save compositing must correctly crop from every display that intersects the selection and stitch the result into a single image.

## Non-Goals
- Hot-plugging displays while the overlay is running.
- Per-display toolbar. Toolbar will render on the window that owns the selection.

---

## Architecture

### OverlayApp Refactor: Single EventLoop, Multiple Windows

`OverlayApp` is currently a single-window `ApplicationHandler`. It will be refactored into a `MultiWindowApp` that manages a `HashMap<WindowId, WindowState>`.

#### New Types

```rust
struct WindowState {
    window: Window,
    gl: GlContext,
    egui_state: EguiState,
    painter: egui_glow::Painter,
    screen_id: String,
    global_offset: LogicalPoint,
}

struct MultiWindowApp {
    engine: Arc<Mutex<Engine>>,
    frames: Vec<ScreenFrame>,
    windows: HashMap<WindowId, WindowState>,
    egui_ctx: egui::Context,
    frame_textures: Vec<Option<egui::TextureHandle>>,
    start_time: Option<Instant>,
    mock_drag_done: bool,
    focus_attempts: u32,
    modifiers: ModifiersState,
}
```

- `egui_ctx` is shared across all windows so UI state (e.g. selection rectangle, toolbar) is synchronized automatically.
- `frame_textures` is loaded once from `frames` and reused by every window render pass.

#### Window Creation (`resumed`)

`resumed` fires once. Inside it we iterate over `frames` and create one winit window per frame:

| Attribute | Value |
|-----------|-------|
| `inner_size` | `logical_bounds.w × logical_bounds.h` |
| `position` | `logical_bounds.x, logical_bounds.y` |
| `decorations` | `false` |
| `transparent` | `true` |
| `resizable` | `false` |

Each window gets its own `GlContext`, `EguiState`, and `Painter`, initialized with that window's `scale_factor()` so mixed-DPI setups (Retina + non-Retina) render correctly.

Each window also receives the same macOS treatment (`setLevel: 25`, `makeKeyAndOrderFront:`).

### Coordinate System

Global coordinates (`LogicalPoint`) remain the unbounded macOS display-space coordinates produced by `CGDisplayBounds`. Each window translates local cursor positions to global coordinates by adding its `global_offset` before forwarding to `Engine`.

### Event Routing

`ApplicationHandler::window_event` now receives a `WindowId`. `MultiWindowApp` looks up the matching `WindowState` to:

1. Feed the event to that window's `egui_state.on_window_event()`.
2. If consumed, stop.
3. If not consumed and the event is pointer-related, convert local position to global and forward to `Engine`.
4. For `RedrawRequested`, render only in the matching window.

### Cross-Display Drag

When the user presses the mouse in window A and drags into window B, winit/macOS typically continues delivering `CursorMoved` to the window that captured the press (A) until release. However, because we convert **local → global** before calling `Engine::on_mouse_drag`, the global coordinates can naturally cross into another display's logical bounds.

The overlay on window B will still redraw every frame (its own `about_to_wait` requests redraw), and because it reads the same `Engine` state and `selection`, it will render the correct unmask region on B **without ever needing B's winit to see the mouse events**.

### Locking Non-Primary Windows in Editing State

When `Engine` transitions to `Editing`, two things happen during the next render pass:

1. `MultiWindowApp` determines which window contains the selection center (`selection.x + selection.w/2`, `selection.y + selection.h/2`). That window is designated **active**.
2. For every other window, we call `setIgnoresMouseEvents:YES` on its `NSWindow` via objc. This makes them display-only: they still show the unmasked selection and any layers, but they no longer receive mouse or keyboard input.

If the user cancels (Esc), all windows restore interactivity and return to `OverlayRunning`.

### Save / Composite Changes (`save.rs`)

`composite_image` will be rewritten to support multi-screen selections:

```rust
pub fn composite_image(frames: &[ScreenFrame], editor: &EditorState) -> Result<RgbaImage, String> {
    let selection = editor.selection.ok_or("No selection")?;

    // 1. Find every frame that intersects the selection.
    let intersecting: Vec<&ScreenFrame> = frames.iter()
        .filter(|f| f.logical_bounds.intersects(selection))
        .collect();

    if intersecting.is_empty() {
        return Err("No frame for selection".into());
    }

    // 2. Compute the union bounding box of all intersections in global logical space.
    let union_rect = ...; // min/max x/y across all intersecting bounds clipped to selection

    // 3. Create an output image sized to the union in *physical* pixels.
    let out_w = (union_rect.w * dominant_dpi).ceil() as u32;
    let out_h = (union_rect.h * dominant_dpi).ceil() as u32;
    let mut output = RgbaImage::new(out_w, out_h);

    // 4. For each intersecting frame, crop its image to the overlap region and paste into output.
    for frame in intersecting {
        let overlap = intersect_rects(selection, frame.logical_bounds);
        let local_logical = global_to_local(overlap, frame.logical_bounds);
        let physical = rect_logical_to_physical(local_logical, frame.dpi_scale);
        let cropped = imageops::crop_imm(&frame.image, physical.x as u32, ...).to_image();
        let offset_in_output = global_to_local(overlap, union_rect);
        let output_offset_physical = rect_logical_to_physical(offset_in_output, dominant_dpi);
        imageops::overlay(&mut output, &cropped, output_offset_physical.x as i64, ...);
    }

    // 5. Apply mosaic layers (same as today, on the final output image).
    ...

    Ok(output)
}
```

- `dominant_dpi` is the DPI of the display that contains the selection center. This avoids scaling artifacts for the bulk of the selection.
- If adjacent displays have different DPIs, the smaller-cropped piece is scaled during `overlay` so it aligns correctly in the stitched image.

### Engine Changes

- `Engine::on_mouse_up` currently takes a `screen_id` argument but only uses it for the click-to-select-window fallback path. It will be changed to take **global** `LogicalPoint` and compute which screen the point belongs to internally.
- `Engine::save` signature unchanged; it delegates to the updated `composite_image`.

### Testing & Validation

1. **Unit test** `save.rs` with mock frames at `(0,0,1000,500)` and `(1000,0,1000,500)` and a selection that straddles `x=1000`. Assert output dimensions and that both source colors appear.
2. **E2E (mock drag)** will be extended with two mock frames. `SCREENSHOT_TEST_MOCK_DRAG` coordinates will span from frame 1 into frame 2, then save, and the output file dimensions will be asserted.
3. **cargo test** for all existing tests must continue passing after the refactor.

---

## Open Decisions

| Decision | Value | Rationale |
|----------|-------|-----------|
| `dominant_dpi` for stitching | DPI of display containing selection center | Minimizes resampling for the primary content; minor display will be scaled only for its cropped sliver. |
| Toolbar placement | Renders on the active window only, positioned just below selection | Same behavior as today, but local to the active window's coordinate space. |
| Cancel behavior | All windows restore `ignoresMouseEvents:NO` and return to `OverlayRunning` | Consistent with current single-window behavior. |
