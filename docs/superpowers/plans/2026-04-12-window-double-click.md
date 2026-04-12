# Window Double-Click Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add double-click support on detected windows during overlay so the window is immediately selected as the screenshot region.

**Architecture:** Add a small `Engine::select_hovered_window()` method that bridges the existing `hovered_window` state to `select_region()`. Trigger it from the egui overlay layer via `pointer.button_double_clicked(egui::PointerButton::Primary)` in the `OverlayRunning` state. This keeps the overlay thin and reuses existing state machine transitions.

**Tech Stack:** Rust, napi-rs, egui, winit, cargo

---

## Files

1. **`crates/screenshot-core/src/core/engine.rs`** — Add `select_hovered_window()` method and unit tests.
2. **`crates/screenshot-core/src/overlay/app.rs`** — Add `pointer.button_double_clicked(egui::PointerButton::Primary)` trigger inside the `OverlayRunning` branch.

---

### Task 1: Add `select_hovered_window()` to `Engine`

**Files:**
- Modify: `crates/screenshot-core/src/core/engine.rs`

- [ ] **Step 1: Add `select_hovered_window()` method after `on_mouse_move()`**

Find the `on_mouse_move` method in `engine.rs` and add the new method immediately after it:

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

- [ ] **Step 2: Commit the method**

```bash
git add crates/screenshot-core/src/core/engine.rs
git commit -m "$(cat <<'EOF'
feat(engine): add select_hovered_window helper

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: Add unit tests for `select_hovered_window()`

**Files:**
- Modify: `crates/screenshot-core/src/core/engine.rs` (in the `mod tests` block at the bottom)

- [ ] **Step 1: Add three unit tests before the closing brace of `mod tests`**

Append these tests at the end of the existing `mod tests` block, just before its closing `}`:

```rust
    #[test]
    fn select_hovered_window_enters_editing() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::OverlayRunning;
        engine.screens = vec![
            ScreenInfo {
                id: "main".into(),
                name: "Main".into(),
                logical_bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
                dpi_scale: 1.0,
            },
        ];
        let window = DetectedWindow {
            id: "w1".into(),
            title: "Test Window".into(),
            bounds: Rect::new(100.0, 100.0, 400.0, 300.0),
            owner_pid: 42,
            z_order: 1,
        };
        engine.hovered_window = Some(window.clone());

        engine.select_hovered_window();

        assert!(matches!(engine.state, EngineState::Editing));
        assert_eq!(engine.editor.selection, Some(window.bounds));
        let event = engine.event_bus.try_recv();
        assert!(
            matches!(event, Some(EngineEvent::RegionSelected { screen_id, rect }) if screen_id == "main" && rect == window.bounds)
        );
    }

    #[test]
    fn select_hovered_window_noop_without_hover() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::OverlayRunning;
        engine.hovered_window = None;

        engine.select_hovered_window();

        assert!(matches!(engine.state, EngineState::OverlayRunning));
        assert!(engine.editor.selection.is_none());
        assert!(engine.event_bus.try_recv().is_none());
    }

    #[test]
    fn select_hovered_window_noop_in_editing() {
        let mut engine = Engine::new(
            "/tmp/test.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        engine.state = EngineState::Editing;
        engine.editor.selection = Some(Rect::new(0.0, 0.0, 500.0, 500.0));
        let window = DetectedWindow {
            id: "w1".into(),
            title: "Test Window".into(),
            bounds: Rect::new(100.0, 100.0, 400.0, 300.0),
            owner_pid: 42,
            z_order: 1,
        };
        engine.hovered_window = Some(window);

        engine.select_hovered_window();

        assert!(matches!(engine.state, EngineState::Editing));
        assert_eq!(engine.editor.selection, Some(Rect::new(0.0, 0.0, 500.0, 500.0)));
        assert!(engine.event_bus.try_recv().is_none());
    }
```

- [ ] **Step 2: Run the new tests to verify they pass**

```bash
cargo test --workspace select_hovered_window
```

Expected output:
```
running 3 tests
test core::engine::tests::select_hovered_window_enters_editing ... ok
test core::engine::tests::select_hovered_window_noop_in_editing ... ok
test core::engine::tests::select_hovered_window_noop_without_hover ... ok
```

- [ ] **Step 3: Run full Rust test suite**

```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 4: Commit the tests**

```bash
git add crates/screenshot-core/src/core/engine.rs
git commit -m "$(cat <<'EOF'
test(engine): add tests for select_hovered_window

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Wire up double-click in the overlay

**Files:**
- Modify: `crates/screenshot-core/src/overlay/app.rs`

- [ ] **Step 1: Add the double-click trigger in `OverlayRunning`**

Locate the `OverlayRunning` match arm in `ScreenshotApp::update()`. Find the block that handles pointer events (around lines 84-103). After the existing `if pointer.any_released()` block and before the `match &mut engine.state` line, insert:

```rust
            if pointer.button_double_clicked(egui::PointerButton::Primary) {
                engine.select_hovered_window();
            }
```

The surrounding context should look like this after insertion:

```rust
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
```

- [ ] **Step 2: Build the workspace to check for compilation errors**

```bash
cargo build --workspace
```

Expected: builds successfully with no errors.

- [ ] **Step 3: Run Rust tests again**

```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 4: Commit the overlay change**

```bash
git add crates/screenshot-core/src/overlay/app.rs
git commit -m "$(cat <<'EOF'
feat(overlay): double-click detected window to select it

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Smoke test via Node.js

- [ ] **Step 1: Build the native addon**

```bash
npm run build
```

Expected: build succeeds and generates `dist/` artifacts.

- [ ] **Step 2: Run the Node smoke test**

```bash
npm test
```

Expected: smoke test passes.

- [ ] **Step 3: Commit if any dist changes are generated**

If `dist/` changed as a result of `npm run build`:

```bash
git add dist/
git commit -m "$(cat <<'EOF'
chore: rebuild dist for window double-click selection

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Self-Review Checklist

1. **Spec coverage:**
   - `Engine::select_hovered_window()` added ✅ (Task 1)
   - Unit tests for the new method ✅ (Task 2)
   - `pointer.button_double_clicked(egui::PointerButton::Primary)` trigger in overlay ✅ (Task 3)
   - Verification via `cargo test --workspace` and `npm test` ✅ (Tasks 2, 4)

2. **Placeholder scan:**
   - No TBD/TODO/fill-in-details found.
   - Each step includes exact code or exact commands.
   - Tests include complete assertions.

3. **Type consistency:**
   - `select_hovered_window` uses `EngineState::OverlayRunning` and `DetectedWindow` bounds just like the spec.
   - `pointer.button_double_clicked(egui::PointerButton::Primary)` is the egui API call used in the overlay.
   - Event assertions match `EngineEvent::RegionSelected { screen_id, rect }`.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-12-window-double-click.md`. Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
