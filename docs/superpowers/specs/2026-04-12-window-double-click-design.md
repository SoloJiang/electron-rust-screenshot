# Window Double-Click Selection Design

## Summary
Add double-click support in the `OverlayRunning` state so that when a user double-clicks a detected window, it is immediately selected as the screenshot region and the engine transitions to `Editing` state.

## Background
The screenshot tool already enumerates on-screen windows using `CGWindowListCopyWindowInfo` and highlights the hovered window with a blue border. A single click-with-minimal-movement also auto-selects the window underneath. Users expect a faster, more explicit gesture—double-click—to select a window directly.

## Design

### 1. Interaction Flow
While in `OverlayRunning` state:
1. Mouse movement continues to update `hovered_window` (existing behavior).
2. When the user double-clicks, `egui::PointerState::double_clicked()` returns `true`.
3. The overlay calls `engine.select_hovered_window()`.
4. The engine computes the screen ID from the hovered window's top-left corner and calls `select_region(screen_id, win.bounds)`.
5. State transitions to `Editing`; toolbar appears on the display containing the selection center.

### 2. API Changes

#### `Engine::select_hovered_window`
Add a new method to `Engine` in `crates/screenshot-core/src/core/engine.rs`:

```rust
pub fn select_hovered_window(&mut self) {
    if let (EngineState::OverlayRunning, Some(win)) = (&self.state, self.hovered_window.clone()) {
        let screen_id = self
            .screen_at_point(LogicalPoint::new(win.bounds.x, win.bounds.y))
            .unwrap_or_else(|| "primary".to_string());
        self.select_region(screen_id, win.bounds);
    }
}
```

Rules:
- Only acts when `state == OverlayRunning`.
- No-op if `hovered_window` is `None`.
- Uses the same screen-at-point logic as `on_mouse_up` for consistency.

#### `ScreenshotApp::update` trigger
In `crates/screenshot-core/src/overlay/app.rs`, inside the `OverlayRunning` match arm, after existing pointer handling, add:

```rust
if pointer.double_clicked() {
    engine.select_hovered_window();
}
```

### 3. Conflict Resolution
`egui` consumes a double-click as a single gesture. The second `mouse_up` still fires, but by that time `select_hovered_window` has already moved the engine into `Editing` state. The existing `on_mouse_up` checks `matches!(self.state, EngineState::OverlayRunning)` before acting, so it naturally becomes a no-op. No duplicated selection can occur.

### 4. Testing

#### Unit tests (Rust)
In `engine.rs`:
- `select_hovered_window_enters_editing`: set `OverlayRunning`, assign `hovered_window`, call `select_hovered_window`, assert `Editing` state, correct `editor.selection`, and `RegionSelected` event.
- `select_hovered_window_noop_without_hover`: set `OverlayRunning`, leave `hovered_window` as `None`, call method, assert state unchanged and no event emitted.
- `select_hovered_window_noop_in_editing`: set `Editing`, assign `hovered_window`, call method, assert state unchanged.

#### Manual / E2E verification (macOS)
- `cargo test --workspace` passes with new tests.
- `npm run build && npm test` passes.
- Manual overlay run: double-clicking a visible window enters editing mode; single-click drag and single-point auto-select continue to work.

### 5. Files Changed
1. `crates/screenshot-core/src/core/engine.rs` — add `select_hovered_window()` and unit tests.
2. `crates/screenshot-core/src/overlay/app.rs` — add `pointer.double_clicked()` trigger in `OverlayRunning`.

## Decision Log
- **Chosen approach**: Detect double-click in the egui layer rather than winit or a manual timer. This minimizes code, uses egui's built-in thresholds, and avoids adding extra state machines.
- **Engine encapsulation**: Exposed `select_hovered_window()` on `Engine` instead of inlining the logic in `app.rs` to keep the overlay layer thin and preserve separation of concerns.
