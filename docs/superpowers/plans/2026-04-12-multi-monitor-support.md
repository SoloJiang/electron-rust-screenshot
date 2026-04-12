# Multi-Monitor Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the macOS screenshot overlay to create one window per display, support cross-display free selection, lock inactive displays during editing, and composite saved images from all intersecting screens.

**Architecture:** Convert `OverlayApp` from a single-window `ApplicationHandler` into `MultiWindowApp` managing a `HashMap<WindowId, WindowState>`. Each display gets its own winit `Window`, `GlContext`, `EguiState`, and `Painter`, but all share one `egui::Context` and one `Arc<Mutex<Engine>>`. Mouse events are translated from window-local to global coordinates so dragging across windows works naturally. `save.rs` is updated to crop-and-stitch from multiple frames that intersect the selection.

**Tech Stack:** Rust, winit, egui, egui_glow, glutin, core-graphics, image crate

---

## File Mapping

| File | Responsibility |
|------|----------------|
| `src/core/types.rs` | Add `Rect::intersection` helper used by `save.rs`. |
| `src/core/engine.rs` | Add `screen_at_point` helper; change `on_mouse_up` to resolve `screen_id` internally. |
| `tests/engine_integration.rs` | Update existing tests for new `on_mouse_up` signature. |
| `src/overlay/save.rs` | Rewrite `composite_image` and add helpers for multi-screen cropping and stitching. |
| `src/overlay/app.rs` | Remove `window_offset` from `ScreenshotApp` struct; pass it as an argument to `update`. |
| `src/overlay/manager.rs` | Replace `OverlayApp` with `MultiWindowApp` + `WindowState`; handle multi-window event routing, rendering, and interactivity locking. |

---

### Task 1: Add `Rect::intersection` helper

**Files:**
- Modify: `src/core/types.rs`
- Test: inline in `src/core/types.rs` `tests` module

- [ ] **Step 1: Add `intersection` to `Rect`**

Insert into `impl Rect` between `intersects` and `is_empty`:

```rust
    pub fn intersection(&self, other: Rect) -> Option<Rect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w);
        let y2 = (self.y + self.h).min(other.y + other.h);
        let w = x2 - x1;
        let h = y2 - y1;
        if w > 0.0 && h > 0.0 {
            Some(Rect::new(x1, y1, w, h))
        } else {
            None
        }
    }
```

- [ ] **Step 2: Add unit test**

Append to the `tests` module at the bottom of `src/core/types.rs`:

```rust
    #[test]
    fn rect_intersection() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let inter = a.intersection(b).unwrap();
        assert_eq!(inter.x, 50.0);
        assert_eq!(inter.y, 50.0);
        assert_eq!(inter.w, 50.0);
        assert_eq!(inter.h, 50.0);
    }

    #[test]
    fn rect_intersection_none() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert!(a.intersection(b).is_none());
    }
```

- [ ] **Step 3: Run test**

Run: `cargo test rect_intersection`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/core/types.rs
git commit -m "feat(core): add Rect::intersection helper for multi-monitor composites"
```

---

### Task 2: Engine resolves screen_id from coordinates

**Files:**
- Modify: `src/core/engine.rs`
- Modify: `src/overlay/app.rs`
- Modify: `tests/engine_integration.rs`

- [ ] **Step 1: Write failing test for `screen_at_point` and `on_mouse_up` auto-detection**

Append to the `tests` module in `src/core/engine.rs`:

```rust
    #[test]
    fn engine_screen_at_point_finds_correct_screen() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.screens = vec![
            ScreenInfo {
                id: "left".into(),
                name: "Left".into(),
                logical_bounds: Rect::new(0.0, 0.0, 1000.0, 500.0),
                dpi_scale: 2.0,
            },
            ScreenInfo {
                id: "right".into(),
                name: "Right".into(),
                logical_bounds: Rect::new(1000.0, 0.0, 1000.0, 500.0),
                dpi_scale: 2.0,
            },
        ];
        assert_eq!(engine.screen_at_point(LogicalPoint::new(100.0, 100.0)), Some("left".into()));
        assert_eq!(engine.screen_at_point(LogicalPoint::new(1100.0, 100.0)), Some("right".into()));
        assert_eq!(engine.screen_at_point(LogicalPoint::new(9999.0, 9999.0)), None);
    }

    #[test]
    fn engine_mouse_up_auto_selects_screen() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.screens = vec![
            ScreenInfo {
                id: "left".into(),
                name: "Left".into(),
                logical_bounds: Rect::new(0.0, 0.0, 1000.0, 500.0),
                dpi_scale: 1.0,
            },
        ];
        engine.state = EngineState::OverlayRunning;
        // trigger free-select
        engine.on_mouse_down(LogicalPoint::new(10.0, 10.0));
        engine.on_mouse_drag(LogicalPoint::new(100.0, 100.0));
        engine.on_mouse_up(LogicalPoint::new(100.0, 100.0));
        assert!(matches!(engine.state, EngineState::Editing));
    }
```

Run: `cargo test engine_screen_at_point`
Expected: FAIL (method not found)

- [ ] **Step 2: Implement `screen_at_point` and update `on_mouse_up`**

In `impl Engine` in `src/core/engine.rs`, insert the helper before `on_mouse_up`:

```rust
    pub fn screen_at_point(&self, pos: LogicalPoint) -> Option<String> {
        self.screens
            .iter()
            .find(|s| s.logical_bounds.contains(pos))
            .map(|s| s.id.clone())
    }

    pub fn on_mouse_up(&mut self, pos: LogicalPoint) {
        let screen_id = self
            .screen_at_point(pos)
            .unwrap_or_else(|| "primary".to_string());
        if let EngineState::FreeSelecting { start, .. } = self.state {
            let dx = (pos.x - start.x).abs();
            let dy = (pos.y - start.y).abs();
            if dx < 4.0 && dy < 4.0 && dx * dy < 16.0 {
                if let Some(detector) = &self.detector {
                    if let Some(win) = detector.hit_test(start) {
                        let rect = win.bounds;
                        self.select_region(screen_id, rect);
                        return;
                    }
                }
                self.state = EngineState::OverlayRunning;
            } else {
                let rect = Rect::new(start.x.min(pos.x), start.y.min(pos.y), dx, dy);
                self.select_region(screen_id, rect);
            }
        }
    }
```

Remove the old `pub fn on_mouse_up(&mut self, screen_id: String, pos: LogicalPoint)` block entirely.

- [ ] **Step 3: Fix callers of `on_mouse_up`**

In `src/overlay/app.rs` line 102, change:

```rust
                    engine.on_mouse_up(
                        "primary".into(),
                        LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y),
                    );
```

to:

```rust
                    engine.on_mouse_up(
                        LogicalPoint::new(pos.x as f64 + offset.x, pos.y as f64 + offset.y),
                    );
```

In `tests/engine_integration.rs`, search for any `on_mouse_up` calls and remove the first `"..."` argument. (If the file does not exist or has no calls, skip.)

- [ ] **Step 4: Run tests**

Run: `cargo test engine_screen_at_point engine_mouse_up_auto_selects_screen`
Expected: PASS

Run: `cargo test`
Expected: all tests in `src/core/engine.rs` PASS; possible failures later in integration tests that call the old signature.

- [ ] **Step 5: Fix any remaining integration-test callers**

Search for compilation errors with:

Run: `cargo test`

If compilation fails in `tests/` due to old `on_mouse_up` signature, fix them by removing the hard-coded `screen_id` string argument.

- [ ] **Step 6: Commit**

```bash
git add src/core/engine.rs src/overlay/app.rs tests/
git commit -m "feat(engine): resolve screen_id from global coordinates in on_mouse_up"
```

---

### Task 3: Multi-screen `composite_image` in `save.rs`

**Files:**
- Modify: `src/overlay/save.rs`

- [ ] **Step 1: Write failing test for cross-screen composite**

Append to the `tests` module in `src/overlay/save.rs`:

```rust
    #[test]
    fn composite_cross_screen_stitches_correctly() {
        let mut img1 = RgbaImage::new(100, 100);
        for p in img1.pixels_mut() {
            *p = Rgba([255, 0, 0, 255]);
        }
        let mut img2 = RgbaImage::new(100, 100);
        for p in img2.pixels_mut() {
            *p = Rgba([0, 0, 255, 255]);
        }
        let frames = vec![
            ScreenFrame {
                screen_id: "left".into(),
                logical_bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
                dpi_scale: 1.0,
                image: img1,
            },
            ScreenFrame {
                screen_id: "right".into(),
                logical_bounds: Rect::new(100.0, 0.0, 100.0, 100.0),
                dpi_scale: 1.0,
                image: img2,
            },
        ];
        let mut editor = EditorState::new(Color::new(255, 0, 0, 255), 3.0, 8.0);
        // selection straddles the boundary
        editor.selection = Some(Rect::new(50.0, 25.0, 100.0, 50.0));
        let out = composite_image(&frames, &editor).unwrap();
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 50);
        // left half should be red, right half blue
        assert_eq!(*out.get_pixel(10, 10), Rgba([255, 0, 0, 255]));
        assert_eq!(*out.get_pixel(60, 10), Rgba([0, 0, 255, 255]));
    }
```

Run: `cargo test composite_cross_screen_stitches_correctly`
Expected: FAIL (function not found or behavior mismatch)

- [ ] **Step 2: Add helpers `intersect_rects` and `global_to_local`**

At the top of `src/overlay/save.rs` (after imports, before `composite_image`):

```rust
fn intersect_rects(a: crate::core::types::Rect, b: crate::core::types::Rect) -> Option<crate::core::types::Rect> {
    a.intersection(b)
}

fn global_to_local(r: crate::core::types::Rect, bounds: crate::core::types::Rect) -> crate::core::types::Rect {
    crate::core::types::Rect::new(
        r.x - bounds.x,
        r.y - bounds.y,
        r.w,
        r.h,
    )
}
```

- [ ] **Step 3: Rewrite `composite_image`**

Replace the entire body of `pub fn composite_image` with:

```rust
pub fn composite_image(frames: &[ScreenFrame], editor: &EditorState) -> Result<RgbaImage, String> {
    let selection = editor.selection.ok_or("No selection")?;

    let intersecting: Vec<&ScreenFrame> = frames
        .iter()
        .filter(|f| f.logical_bounds.intersects(selection))
        .collect();

    if intersecting.is_empty() {
        return Err("No frame for selection".into());
    }

    // Determine dominant DPI from the screen that contains the selection center.
    let cx = selection.x + selection.w / 2.0;
    let cy = selection.y + selection.h / 2.0;
    let dominant_dpi = frames
        .iter()
        .find(|f| f.logical_bounds.contains(LogicalPoint::new(cx, cy)))
        .map(|f| f.dpi_scale)
        .unwrap_or(1.0);

    // Compute union of all clipped intersections in global logical space.
    let union_rect = {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        for frame in &intersecting {
            if let Some(clip) = intersect_rects(selection, frame.logical_bounds) {
                min_x = min_x.min(clip.x);
                min_y = min_y.min(clip.y);
                max_x = max_x.max(clip.x + clip.w);
                max_y = max_y.max(clip.y + clip.h);
            }
        }
        crate::core::types::Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    };

    let out_w = (union_rect.w * dominant_dpi).ceil().max(0.0) as u32;
    let out_h = (union_rect.h * dominant_dpi).ceil().max(0.0) as u32;
    let mut output = RgbaImage::new(out_w, out_h);

    for frame in intersecting {
        let Some(overlap) = intersect_rects(selection, frame.logical_bounds) else {
            continue;
        };
        let local_logical = global_to_local(overlap, frame.logical_bounds);
        let physical = crate::core::dpi::rect_logical_to_physical(local_logical, frame.dpi_scale);
        let x = physical.x.max(0.0) as u32;
        let y = physical.y.max(0.0) as u32;
        let w = physical.w.max(0.0) as u32;
        let h = physical.h.max(0.0) as u32;

        if x + w > frame.image.width() || y + h > frame.image.height() {
            return Err("Selection out of bounds".into());
        }

        let cropped = image::imageops::crop_imm(&frame.image, x, y, w, h).to_image();

        let offset_in_union = global_to_local(overlap, union_rect);
        let output_offset = crate::core::dpi::rect_logical_to_physical(offset_in_union, dominant_dpi);
        let ox = output_offset.x as i64;
        let oy = output_offset.y as i64;

        // If DPIs differ, scale the cropped piece to fit into the dominant-dpi canvas.
        if (frame.dpi_scale - dominant_dpi).abs() > f64::EPSILON {
            let target_w = (overlap.w * dominant_dpi).ceil().max(0.0) as u32;
            let target_h = (overlap.h * dominant_dpi).ceil().max(0.0) as u32;
            let scaled = image::imageops::resize(
                &cropped,
                target_w,
                target_h,
                image::imageops::FilterType::Lanczos3,
            );
            image::imageops::overlay(&mut output, &scaled, ox, oy);
        } else {
            image::imageops::overlay(&mut output, &cropped, ox, oy);
        }
    }

    for layer in &editor.layers {
        if let Layer::MosaicPath {
            points, block_size, ..
        } = layer
        {
            apply_mosaic(&mut output, points, *block_size, dominant_dpi);
        }
    }

    Ok(output)
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test composite_cross_screen_stitches_correctly`
Expected: PASS

Run: `cargo test composite_with_no_layers`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/overlay/save.rs
git commit -m "feat(save): support multi-screen composite and cross-display stitching"
```

---

### Task 4: Remove `window_offset` from `ScreenshotApp`

**Files:**
- Modify: `src/overlay/app.rs`
- Modify: `src/overlay/manager.rs`

- [ ] **Step 1: Update `ScreenshotApp` struct and `new`**

In `src/overlay/app.rs`:

Remove the `window_offset` field from `ScreenshotApp`:

```rust
pub struct ScreenshotApp {
    pub engine: Arc<Mutex<Engine>>,
    pub frame_textures: Vec<Option<egui::TextureHandle>>,
    pub frames: Vec<ScreenFrame>,
}
```

Change `new`:

```rust
    pub fn new(engine: Arc<Mutex<Engine>>, frames: Vec<ScreenFrame>) -> Self {
        Self {
            engine,
            frame_textures: vec![None; frames.len()],
            frames,
        }
    }
```

- [ ] **Step 2: Update `update` signature to accept `offset`**

Change:

```rust
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut egui::Frame,
        frame_time_ms: f64,
        egui_paint_ms: f64,
    ) {
```

to:

```rust
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut egui::Frame,
        offset: LogicalPoint,
        frame_time_ms: f64,
        egui_paint_ms: f64,
    ) {
```

Inside `update`, replace the line:

```rust
        let offset = self.window_offset;
```

with nothing (parameter is already in scope).

- [ ] **Step 3: Update `draw_screenshot_textures` and `draw_unmasked_region` to take offset**

Change signatures:

```rust
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
```

Update all call sites inside `update` to pass `offset`:
- `self.draw_screenshot_textures(ui.painter(), offset);` (two places: OverlayRunning and Editing background)
- `self.draw_unmasked_region(ui.painter(), win.bounds, offset);`
- `self.draw_unmasked_region(ui.painter(), sel, offset);` (FreeSelecting)
- `self.draw_unmasked_region(ui.painter(), sel, offset);` (Editing)
- `draw_layer(ui.painter(), layer, offset);` (already passes offset)
- `draw_layer(ui.painter(), preview, offset);` (already passes offset)

- [ ] **Step 4: Update manager.rs caller temporarily**

In `src/overlay/manager.rs`, inside `resumed`, change:

```rust
        let mut app = ScreenshotApp::new(Arc::clone(&self.engine), frames, offset);
```

to:

```rust
        let mut app = ScreenshotApp::new(Arc::clone(&self.engine), frames);
```

And in `RedrawRequested` change:

```rust
                    app.update(
                        ctx,
                        &mut egui::Frame::none(),
                        self.last_frame_time_ms,
                        self.last_egui_paint_ms,
                    );
```

to:

```rust
                    app.update(
                        ctx,
                        &mut egui::Frame::none(),
                        offset,
                        self.last_frame_time_ms,
                        self.last_egui_paint_ms,
                    );
```

For the existing single-window setup `offset` in `resumed` is already defined as `LogicalPoint::new(rect.x, rect.y)`. This keeps the single-window build compiling until we replace it fully in Task 5.

- [ ] **Step 5: Compile check**

Run: `cargo check`
Expected: PASS (modulo warnings)

- [ ] **Step 6: Commit**

```bash
git add src/overlay/app.rs src/overlay/manager.rs
git commit -m "refactor(overlay): make ScreenshotApp offset per-window rather than per-instance"
```

---

### Task 5: MultiWindowApp skeleton and window creation

**Files:**
- Modify: `src/overlay/manager.rs`

- [ ] **Step 1: Replace `OverlayApp` struct with `MultiWindowApp` and `WindowState`**

Replace the entire `struct OverlayApp { ... }` block with:

```rust
use std::collections::HashMap;

struct WindowState {
    window: Window,
    gl_context: GlContext,
    egui_state: EguiState,
    painter: egui_glow::Painter,
    screen_id: String,
    global_offset: LogicalPoint,
    screen_bounds: Rect,
    ignores_mouse_events: bool,
    last_frame_time_ms: f64,
    last_egui_paint_ms: f64,
}

struct MultiWindowApp {
    engine: Arc<Mutex<Engine>>,
    egui_ctx: egui::Context,
    screenshot_app: Option<ScreenshotApp>,
    frame_textures: Vec<Option<egui::TextureHandle>>,
    windows: HashMap<winit::window::WindowId, WindowState>,
    start_time: Option<std::time::Instant>,
    mock_drag_done: bool,
    focus_attempts: u32,
    modifiers: winit::keyboard::ModifiersState,
}
```

Update `OverlayApp::new` to become `MultiWindowApp::new`:

```rust
impl MultiWindowApp {
    fn new(engine: Arc<Mutex<Engine>>) -> Self {
        Self {
            engine,
            egui_ctx: egui::Context::default(),
            screenshot_app: None,
            frame_textures: Vec::new(),
            windows: HashMap::new(),
            start_time: None,
            mock_drag_done: false,
            focus_attempts: 0,
            modifiers: winit::keyboard::ModifiersState::empty(),
        }
    }
}
```

- [ ] **Step 2: Update `OverlayManager::run` to spawn `MultiWindowApp`**

Change `OverlayManager::run`:

```rust
    pub fn run(self) {
        let event_loop = EventLoop::new().expect("Failed to create event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = MultiWindowApp::new(self.engine);
        // Pre-populate frames into the app so it can create windows in resumed
        app.screenshot_app = Some(ScreenshotApp::new(
            Arc::clone(&self.engine),
            self.frames.clone(),
        ));
        app.screenshot_app.as_mut().unwrap().load_screenshot_textures(&app.egui_ctx);
        app.frame_textures = app.screenshot_app.as_ref().unwrap().frame_textures.clone();
        let _ = event_loop.run_app(&mut app);
    }
```

Wait, `ScreenshotApp::new` currently takes `engine` and `frames`. We set `frame_textures` inside it. Then after `load_screenshot_textures`, we clone them into `MultiWindowApp`. This is fine. Actually a simpler approach: keep `frames` in `MultiWindowApp` temporarily, create `screenshot_app` inside `resumed` once all GL contexts exist. But `load_screenshot_textures` only needs `egui_ctx`, not GL context. So we can load them before the event loop starts as above.

Alternatively, to keep `run` simple, store `frames` on `MultiWindowApp` and build `screenshot_app` in `resumed`. Let's do that to avoid lifetime issues with Arc clones. Revise `MultiWindowApp` to also have `pending_frames: Vec<ScreenFrame>`, and `resumed` will take ownership.

Revised `MultiWindowApp` fields add `pending_frames: Vec<ScreenFrame>`.
Revised `new` initializes `pending_frames: Vec::new()`.
Revised `run`:

```rust
    pub fn run(self) {
        let event_loop = EventLoop::new().expect("Failed to create event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = MultiWindowApp::new(self.engine);
        app.pending_frames = self.frames;
        let _ = event_loop.run_app(&mut app);
    }
```

And `resumed` does:

```rust
        let frames = std::mem::take(&mut self.pending_frames);
        let mut screenshot_app = ScreenshotApp::new(Arc::clone(&self.engine), frames.clone());
        screenshot_app.load_screenshot_textures(&self.egui_ctx);
        self.frame_textures = screenshot_app.frame_textures.clone();
        self.screenshot_app = Some(screenshot_app);
```

- [ ] **Step 3: Implement `resumed` for `MultiWindowApp`**

Replace the existing `fn resumed` inside `impl ApplicationHandler for MultiWindowApp` with:

```rust
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let frames = std::mem::take(&mut self.pending_frames);
        if frames.is_empty() {
            return;
        }
        let mut screenshot_app = ScreenshotApp::new(Arc::clone(&self.engine), frames.clone());
        screenshot_app.load_screenshot_textures(&self.egui_ctx);
        self.frame_textures = screenshot_app.frame_textures.clone();
        self.screenshot_app = Some(screenshot_app);

        for frame in frames {
            let b = frame.logical_bounds;
            let window_attributes = Window::default_attributes()
                .with_title("Screenshot Overlay")
                .with_inner_size(winit::dpi::LogicalSize::new(b.w, b.h))
                .with_position(winit::dpi::LogicalPosition::new(b.x, b.y))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false);

            let window = event_loop.create_window(window_attributes).unwrap();

            #[cfg(target_os = "macos")]
            unsafe {
                use objc::class;
                use objc::msg_send;
                use objc::runtime::Object;
                use objc::sel;
                use objc::sel_impl;
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                let policy: i64 = 0; // NSApplicationActivationPolicyRegular
                let _: () = msg_send![ns_app, setActivationPolicy: policy];
                let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
                if let Ok(handle) = window.window_handle() {
                    if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                        let ns_view: *mut Object = appkit.ns_view.as_ptr() as *mut Object;
                        let ns_window: *mut Object = msg_send![ns_view, window];
                        if !ns_window.is_null() {
                            let level: i64 = 25; // NSStatusWindowLevel
                            let _: () = msg_send![ns_window, setLevel: level];
                            let _: () = msg_send![ns_window, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
                        }
                    }
                }
            }

            let gl = unsafe { GlContext::new(&window, event_loop) };
            let egui_state = EguiState::new(
                self.egui_ctx.clone(),
                egui::ViewportId::default(),
                &window,
                Some(window.scale_factor() as f32),
                None,
                None::<usize>,
            );
            let painter = egui_glow::Painter::new(gl.gl.clone(), "", None, true)
                .expect("Failed to create egui_glow Painter");

            let ws = WindowState {
                window,
                gl_context: gl,
                egui_state,
                painter,
                screen_id: frame.screen_id.clone(),
                global_offset: LogicalPoint::new(b.x, b.y),
                screen_bounds: b,
                ignores_mouse_events: false,
                last_frame_time_ms: 0.0,
                last_egui_paint_ms: 0.0,
            };
            let id = ws.window.id();
            self.windows.insert(id, ws);
        }

        self.start_time = Some(std::time::Instant::now());
        for ws in self.windows.values() {
            ws.window.focus_window();
            ws.window.request_redraw();
        }
    }
```

- [ ] **Step 4: Compile check after struct + resumed changes**

Run: `cargo check`
Expected: compile errors around `window_event` and `about_to_wait` because the old single-window methods are gone. This is expected; we will fix them in the next Task.

---

### Task 6: Multi-window event routing and rendering

**Files:**
- Modify: `src/overlay/manager.rs`

- [ ] **Step 1: Implement `window_event` per-window routing**

Replace the entire `fn window_event` inside `impl ApplicationHandler for MultiWindowApp` with:

```rust
    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(ws) = self.windows.get_mut(&window_id) else {
            return;
        };
        let window = &ws.window;
        let response = ws.egui_state.on_window_event(window, &event);
        if response.consumed {
            return;
        }

        let Some(gl) = self.windows.get(&window_id).map(|w| &w.gl_context) else {
            return;
        };
        let Some(app) = self.screenshot_app.as_mut() else {
            return;
        };

        // Borrow ws mutably again after immutable borrows above.
        let ws = self.windows.get_mut(&window_id).unwrap();

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == winit::event::ElementState::Pressed =>
            {
                let is_cmd = self.modifiers.super_key();
                let is_shift = self.modifiers.shift_key();

                if event.logical_key
                    == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
                {
                    self.engine.lock().unwrap().cancel();
                    event_loop.exit();
                }
                if event.logical_key
                    == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter)
                {
                    let frames = &app.frames;
                    self.engine.lock().unwrap().save(frames);
                }

                // Undo / Redo
                if is_cmd && !is_shift {
                    if let winit::keyboard::Key::Character(c) = &event.logical_key {
                        if c.eq_ignore_ascii_case("z") {
                            self.engine.lock().unwrap().editor.undo();
                        }
                    }
                }
                if is_cmd && is_shift {
                    if let winit::keyboard::Key::Character(c) = &event.logical_key {
                        if c.eq_ignore_ascii_case("z") {
                            self.engine.lock().unwrap().editor.redo();
                        }
                    }
                }

                // Copy to clipboard and tool switching (only in Editing state)
                {
                    let mut engine = self.engine.lock().unwrap();
                    if matches!(engine.state, crate::core::engine::EngineState::Editing) {
                        if let winit::keyboard::Key::Character(c) = &event.logical_key {
                            let key = c.as_str();
                            match key {
                                "c" | "C" => {
                                    let frames = &app.frames;
                                    engine.copy_to_clipboard(frames);
                                }
                                "1" => engine.editor.active_tool = crate::core::editor::Tool::Rect,
                                "2" => {
                                    engine.editor.active_tool = crate::core::editor::Tool::Ellipse
                                }
                                "3" => engine.editor.active_tool = crate::core::editor::Tool::Arrow,
                                "4" => engine.editor.active_tool = crate::core::editor::Tool::Brush,
                                "5" => {
                                    engine.editor.active_tool = crate::core::editor::Tool::Mosaic
                                }
                                "6" => engine.editor.active_tool = crate::core::editor::Tool::Text,
                                _ => {}
                            }
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let frame_start = std::time::Instant::now();
                let size = ws.window.inner_size();
                ws.gl_context.resize(size.width, size.height);
                unsafe {
                    use glow::HasContext;
                    ws.gl_context.gl.viewport(0, 0, size.width as i32, size.height as i32);
                    ws.gl_context.gl.clear_color(0.0, 0.0, 0.0, 0.0);
                    ws.gl_context.gl.clear(glow::COLOR_BUFFER_BIT);
                }

                let raw_input = ws.egui_state.take_egui_input(&ws.window);
                let offset = ws.global_offset;
                let full_output = self.egui_ctx.run(raw_input, |ctx| {
                    app.update(
                        ctx,
                        &mut egui::Frame::none(),
                        offset,
                        ws.last_frame_time_ms,
                        ws.last_egui_paint_ms,
                    );
                });
                ws.egui_state.handle_platform_output(&ws.window, full_output.platform_output);

                let clipped_primitives =
                    self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
                let ppp = self.egui_ctx.native_pixels_per_point().unwrap_or(1.0);
                let paint_start = std::time::Instant::now();
                ws.painter.paint_and_update_textures(
                    [size.width, size.height],
                    ppp,
                    &clipped_primitives,
                    &full_output.textures_delta,
                );
                ws.last_egui_paint_ms = paint_start.elapsed().as_secs_f64() * 1000.0;

                ws.gl_context.swap_buffers();
                ws.last_frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
                ws.window.request_redraw();
            }
            _ => {}
        }

        self.update_interactivity();

        if self.engine.lock().unwrap().should_close {
            event_loop.exit();
        }
    }
```

- [ ] **Step 2: Implement `about_to_wait` and `update_interactivity`**

Append these methods to `impl MultiWindowApp` (outside the `ApplicationHandler` impl):

```rust
impl MultiWindowApp {
    fn update_interactivity(&mut self) {
        let engine = self.engine.lock().unwrap();
        let active_id = match engine.state {
            crate::core::engine::EngineState::Editing => {
                engine.editor.selection.and_then(|sel| {
                    let cx = sel.x + sel.w / 2.0;
                    let cy = sel.y + sel.h / 2.0;
                    self.windows.iter().find(|(_, ws)| {
                        ws.screen_bounds
                            .contains(LogicalPoint::new(cx, cy))
                    }).map(|(id, _)| *id)
                })
            }
            _ => None,
        };
        drop(engine);

        for (id, ws) in self.windows.iter_mut() {
            let should_ignore = active_id.map(|a| a != *id).unwrap_or(false);
            if should_ignore != ws.ignores_mouse_events {
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc::msg_send;
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    use objc::runtime::Object;
                    if let Ok(handle) = ws.window.window_handle() {
                        if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                            let ns_view: *mut Object = appkit.ns_view.as_ptr() as *mut Object;
                            let ns_window: *mut Object = msg_send![ns_view, window];
                            if !ns_window.is_null() {
                                let _: () = msg_send![ns_window, setIgnoresMouseEvents: should_ignore];
                            }
                        }
                    }
                }
                ws.ignores_mouse_events = should_ignore;
            }
        }
    }
}
```

Then add `about_to_wait` inside `impl ApplicationHandler for MultiWindowApp`:

```rust
    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        for ws in self.windows.values() {
            ws.window.request_redraw();
            if self.focus_attempts < 60 {
                ws.window.focus_window();
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc::class;
                    use objc::msg_send;
                    use objc::runtime::Object;
                    use objc::sel;
                    use objc::sel_impl;
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    let ns_app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                    let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
                    if let Ok(handle) = ws.window.window_handle() {
                        if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                            let ns_view: *mut Object = appkit.ns_view.as_ptr() as *mut Object;
                            let ns_window: *mut Object = msg_send![ns_view, window];
                            if !ns_window.is_null() {
                                let _: () = msg_send![ns_window, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
                            }
                        }
                    }
                }
            }
        }
        if self.focus_attempts < 60 {
            self.focus_attempts += 1;
        }

        self.update_interactivity();

        if self.engine.lock().unwrap().should_close {
            event_loop.exit();
            return;
        }

        if let Some(start) = self.start_time {
            if !self.mock_drag_done {
                if let Ok(mock_drag) = std::env::var("SCREENSHOT_TEST_MOCK_DRAG") {
                    if start.elapsed().as_secs_f64() > 5.0 {
                        let parts: Vec<f64> = mock_drag
                            .split(',')
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        if parts.len() == 4 {
                            let start_pt = LogicalPoint::new(parts[0], parts[1]);
                            let end_pt = LogicalPoint::new(parts[2], parts[3]);
                            let mut engine = self.engine.lock().unwrap();
                            engine.on_mouse_down(start_pt);
                            engine.on_mouse_drag(end_pt);
                            engine.on_mouse_up(end_pt);
                            if std::env::var("SCREENSHOT_TEST_MOCK_DRAG_NO_SAVE").is_err() {
                                if let Some(app) = &self.screenshot_app {
                                    engine.save(&app.frames);
                                }
                            }
                            self.mock_drag_done = true;
                        }
                    }
                }
            }
            if let Ok(timeout_str) = std::env::var("SCREENSHOT_TEST_TIMEOUT_MS") {
                if let Ok(timeout_ms) = timeout_str.parse::<u64>() {
                    if start.elapsed().as_millis() as u64 > timeout_ms {
                        self.engine.lock().unwrap().cancel();
                        event_loop.exit();
                    }
                }
            }
        }
    }
```

- [ ] **Step 3: Compile check**

Run: `cargo check`
Expected: PASS (or fix any borrow-check / type issues)

- [ ] **Step 4: Run Rust unit + integration tests**

Run: `cargo test`
Expected: All tests PASS. Some E2E tests may still pass because they exercise the Node wrapper; run the smoke test too.

Run: `npm test`
Expected: Node smoke test PASS (it may test basic start/cancel without asserting multi-monitor specifics).

- [ ] **Step 5: Commit**

```bash
git add src/overlay/manager.rs
git commit -m "feat(overlay): implement MultiWindowApp with per-display windows and interactivity locking"
```

---

### Task 7: End-to-end validation

**Files:**
- Modify: none (verification only)

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test`
Expected: PASSes for all Rust tests (including new cross-screen composite test).

- [ ] **Step 2: Run Node smoke test**

Run: `npm test`
Expected: PASS

- [ ] **Step 3: (Optional) Manual build verification**

Run: `npm run build`
Expected: Builds the native addon without errors.

- [ ] **Step 4: Final commit if any uncommitted changes**

If any fixes were made during validation, commit them:

```bash
git add -A
git commit -m "fix(overlay): address edge cases from multi-monitor test validation"
```

---

## Self-Review Checklist

1. **Spec coverage:**
   - One window per display → Task 5 (`resumed` loop)
   - Cross-display drag → Task 6 (local→global coordinate routing in `window_event`) + Task 2 (`on_mouse_up` auto-detection)
   - Lock inactive screens in Editing → Task 6 (`update_interactivity`)
   - Multi-screen save composite → Task 3
   - All covered.

2. **Placeholder scan:** No TBDs, TODOs, or vague instructions.

3. **Type consistency:** `on_mouse_up` takes only `LogicalPoint`. `ScreenshotApp::update` takes `offset: LogicalPoint`. `MultiWindowApp` stores `pending_frames`, `windows`, and `egui_ctx`. All names consistent across tasks.
