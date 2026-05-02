# electron-rust-screenshot

A native screenshot toolkit for Node.js / Electron, built in Rust with [napi-rs](https://napi.rs/).

[English](./README.md) · [中文](./README.zh-CN.md)

## Features

- Full-screen overlay with multi-monitor support and per-display GL context isolation
- Cross-screen drag selection, with logical-coordinate geometry throughout
- Window detection with hover highlighting (R-tree spatial index) and double-click to capture
- In-place editor: rectangle / ellipse / arrow / brush / mosaic / text, with undo / redo
- Mosaic redaction, save-to-disk and one-key clipboard copy
- Synchronous blocking `start()` API designed for Electron's main process

## Platform Support

| Platform | Status |
|----------|--------|
| macOS    | Fully supported (CoreGraphics capture, NSStatusWindowLevel overlay) |
| Windows  | Stub (`WindowsBackend` not yet implemented) |
| Linux/Other | `UnsupportedBackend` — calls return errors |

## Install

```bash
npm install electron-rust-screenshot
```

> The package ships native binaries via `napi-rs`. To build from source you'll need a stable Rust toolchain plus Node.js 20+.

## Build From Source

```bash
git clone <this-repo>
cd electron-rust-screenshot
npm install
npm run build        # produces dist/*.node and dist/index.js
```

## Quick Start

```javascript
const { start } = require('electron-rust-screenshot');

const result = start({
  savePath: '/tmp/shot.png',
  format: 'png',         // 'png' | 'jpg'
  quality: 90,           // 1-100
  defaultColor: '#ff0000',
  defaultSize: 3,
});

console.log(result);
// → { type: 'saved', path: '/tmp/shot.png', copied: false }
// → { type: 'cancelled' }
// → { type: 'error', code: '...', message: '...' }
```

> **Important:** `start()` blocks the calling thread until the overlay closes, because macOS requires the `winit::EventLoop` to live on the main thread. In Electron, call it from the main process.

## Try the Demo

```bash
node scripts/demo.js interactive   # png, red pen
node scripts/demo.js jpg           # jpeg, green pen
node scripts/demo.js clipboard     # png, blue pen
```

## Hotkeys

| Action | Key |
|--------|-----|
| Select region / draw shape | Mouse drag |
| Highlight hovered window | Mouse move |
| Capture hovered window directly | Double-click |
| Switch tool (Rect / Ellipse / Arrow / Brush / Mosaic / Text) | `1`–`6` |
| Undo | `Cmd+Z` |
| Redo | `Cmd+Shift+Z` |
| Copy to clipboard (in editing mode) | `C` |
| Save | `Enter` |
| Cancel | `Esc` |

## API

### `start(config?: ScreenshotConfig): string`

Launches the overlay synchronously. Returns a JSON string describing the final event.

### `ScreenshotConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `savePath` | `string` | `/tmp` | Output path for the saved image |
| `format` | `'png' \| 'jpg'` | `'png'` | Encoding format |
| `quality` | `number` (1–100) | `90` | JPEG quality (PNG ignores) |
| `mosaicBlockSize` | `number` | `8` | Mosaic tile size in logical pixels |
| `defaultColor` | `string` (hex) | `#ff0000` | Default brush / shape color |
| `defaultSize` | `number` | `3` | Default brush / stroke width |
| `locale` | `string` | `zh-CN` | UI locale |
| `showDebugHud` | `boolean` | `false` | Toggle on-screen perf HUD |
| `metricsIntervalMs` | `number` | `500` | Interval between `metrics` events |

### Final event JSON

```jsonc
{ "type": "saved",     "path": "/tmp/shot.png", "copied": false }
{ "type": "cancelled" }
{ "type": "error",     "code": "<error-code>",  "message": "<reason>" }
```

## Project Structure

```
crates/
  screenshot-core/    # platform-agnostic Rust library (rlib)
    src/core/         # engine, capture, editor, events, types, dpi, perf, window detector
    src/overlay/      # winit + egui multi-window overlay, GL helpers, image compositor, toolbar
    src/platform/     # macOS / Windows / unsupported backends
  napi-bindings/      # napi-rs cdylib that wraps the core for Node
    src/bridge/       # ScreenshotConfig, ScreenshotSession, JSON event serializer
dist/                 # build output (.node + JS wrappers)
e2e/                  # Jest + AppleScript E2E tests
scripts/              # demo.js (interactive) and test-smoke.js
```

For the deep-dive architecture (multi-monitor GL isolation, coordinate system, NSWindow setup, composite algorithm), see [`CLAUDE.md`](./CLAUDE.md).

## Development

```bash
cargo test --workspace                                  # Rust unit + integration tests
npm test                                                # Node smoke test
cd e2e && npx jest --runInBand                          # AppleScript E2E suite
cargo fmt --all -- --check                              # CI lint gate
cargo clippy --workspace --all-targets -- -D warnings   # CI lint gate
```

E2E tests inject mouse events via [`cliclick`](https://github.com/BlueM/cliclick), which requires Accessibility permission. In headless / sandboxed environments, set `SCREENSHOT_TEST_MOCK_DRAG=x1,y1,x2,y2` (e.g. `300,300,500,500`) and the overlay will simulate the drag internally about 5 seconds after launch.

## License

ISC
