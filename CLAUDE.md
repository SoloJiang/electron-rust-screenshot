# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a **Rust + Node.js hybrid screenshot tool** built with `napi-rs`. Rust handles all native UI (capture, overlay, editing, saving) via `winit` + `egui_glow`, while Node.js only drives the process through a blocking `start(config)` API and receives serialized events.

## Build Commands

```bash
# Build the native Rust addon (generates .node file and overwrites index.js)
npm run build

# Run Rust unit/integration tests
cargo test

# Run a specific Rust test
cargo test engine_cancel_after_start

# Run smoke test (Node.js)
npm test

# Run AppleScript E2E suite (requires accessibility permissions for cliclick)
cd e2e && npx jest --runInBand

# Run a specific E2E spec
cd e2e && npx jest --runInBand specs/free-select.spec.ts
```

## Critical Architecture Details

### Blocking Synchronous `start()` API

`src/lib.rs` exposes a `#[napi] pub fn start(config: Option<ScreenshotConfig>) -> Result<String>`. **This function blocks the calling thread** because macOS requires `winit::EventLoop` to run on the main thread. The function returns only after the overlay closes, with a JSON string representing the final event (`saved`, `cancelled`, or `error`).

Because `start()` blocks, the E2E runner (`e2e/helpers/runner.ts`) spawns it in a `child_process`, polls for a result JSON temp file, and then kills the process.

### JS Entry Point Stability

`npm run build` auto-generates `index.js` and `native.js`. The stable wrapper is `lib.js`, which imports `native.js` and parses the JSON result. `package.json` points `"main": "lib.js"`. Do not edit `index.js` directly — it will be overwritten on the next build.

### Core Layers

- **`src/lib.rs`** — napi entry point. Sets up `Engine`, runs `PlatformCapture`, then hands control to `OverlayManager` and finally drains `EventBus` for the result.
- **`src/core/`** — Platform-agnostic core:
  - `engine.rs` — State machine (`Idle -> Capturing -> OverlayRunning -> FreeSelecting -> Editing -> Saving`).
  - `capture.rs` — `PlatformCapture` trait.
  - `editor.rs` — `EditorState` with tools, layers, and undo/redo.
  - `events.rs` — `EngineEvent` enum and `EventBus` (crossbeam-channel).
  - `types.rs` — Logical coordinates (`Rect`, `LogicalPoint`), `Color`, `ScreenInfo`, `DetectedWindow`.
- **`src/overlay/`** — UI layer:
  - `manager.rs` — `OverlayManager` / `OverlayApp` (winit `ApplicationHandler`). Handles window creation, macOS `NSWindow` level elevation, keyboard shortcuts (Esc = cancel, Enter = save), and the `egui_glow` render loop.
  - `app.rs` — `ScreenshotApp` (egui app). Renders screenshot texture, selection mask, toolbar, and layers.
  - `gl.rs` — `GlContext` helper using `glutin 0.32` + `glutin-winit 0.5`.
  - `save.rs` — Crops the captured frame to the selected region and applies mosaic before saving to disk.
- **`src/platform/macos/`** — macOS-specific implementations:
  - `capture_cg.rs` — `CGDisplayCreateImage` fallback capture.
  - `capture_sck.rs` — `ScreenCaptureKit` stub (currently falls back to CGDisplay).
  - `window.rs` — `CGWindowListCopyWindowInfo` window enumeration.
  - `clipboard.rs` — `NSPasteboard` image copy.
- **`src/bridge/`** — napi bindings and JS serialization:
  - `config.rs` — `ScreenshotConfig` napi struct.
  - `session.rs` — `ScreenshotSession` napi class (used for async event-emitter style, though the current primary API is the blocking `start()`).
  - `events_js.rs` — Manual JSON serialization of `EngineEvent`.

### Coordinate System

All geometry inside `src/core/` uses **logical coordinates**. Physical pixel conversion happens only at capture time (`CGDisplayCreateImage`) and save time (`dpi.rs`). The overlay window is sized to the primary screen's logical bounds.

### E2E Automation

The E2E suite uses AppleScript shelling out to `cliclick` for mouse/keyboard simulation. Tests run Jest in the `e2e/` directory with `ts-jest`. Important helpers:

- `e2e/helpers/runner.ts` — Spawns `start()` in a child process with `SCREENSHOT_TEST_TIMEOUT_MS`.
- `e2e/helpers/mouse.ts` / `keyboard.ts` — AppleScript wrappers around `cliclick`.
- `e2e/helpers/window.ts` — Opens/closes Safari fixture windows via AppleScript.

**Accessibility permissions are required** for `cliclick` to inject events. If unavailable, the overlay supports a `SCREENSHOT_TEST_MOCK_DRAG` fallback that simulates a drag-and-save internally after 5 seconds.

### Overlay Window Behavior on macOS

`OverlayApp::resumed` creates the window without `Fullscreen::Borderless(None)` (to avoid macOS Space switching issues). Instead it uses the primary display's logical bounds for `inner_size` and `position`. On macOS it additionally uses `objc` to:

1. `NSApplication activateIgnoringOtherApps:YES`
2. `setLevel:NSStatusWindowLevel` (25)
3. `makeKeyAndOrderFront:`

This ensures the overlay receives keyboard events (Esc, Enter) and sits above all other windows.
