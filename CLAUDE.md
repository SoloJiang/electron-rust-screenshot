# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

这是一个基于 `napi-rs` 构建的 Rust + Node.js 混合截图工具。项目采用 Cargo workspace 组织：

- **`crates/screenshot-core/`** — 纯 Rust 库，包含所有 native UI 逻辑（capture、overlay、editing、saving）。编译为 `rlib`。
- **`crates/napi-bindings/`** — 轻量的 `cdylib` crate，通过 `napi-rs` 暴露 Node API。包含 `build.rs` 和 `bridge/` 模块。
- **`crates/harness-protocol/`** — 纯数据 crate，定义 JSONL wire protocol 的 `Command`、`ServerMessage`、`Tier` 等类型，无外部依赖。
- **`crates/validator/`** — Rust 二进制 crate，通过本地 socket 协议驱动引擎，解析 TOML spec 并输出测试报告。
- **`dist/`** — 所有 napi 构建产物（`.node`、`index.js`、`index.d.ts`、`lib.js`）。
- **`e2e/`** — 独立的 Node.js 项目，使用 Jest + AppleScript 进行 E2E 测试。
- **`validator-specs/`** — TOML 格式的自动化测试场景，由 `validator` 二进制执行。

## 常用命令

```bash
# 构建 native Rust addon（产物输出到 dist/）
npm run build

# 运行 workspace 内所有 Rust 单元/集成测试
cargo test --workspace

# 运行某个特定 Rust 测试
cargo test engine_cancel_after_start

# 运行 smoke test（Node.js）
npm test

# 运行 AppleScript E2E 套件（需要为 cliclick 开启 accessibility 权限）
cd e2e && npx jest --runInBand

# 运行某个特定 E2E spec
cd e2e && npx jest --runInBand specs/free-select.spec.ts

# CI 必跑的 lint gate（提交前最好本地跑一次）
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# 手动触发 overlay 进行交互验证（mode: interactive | jpg | clipboard）
node scripts/demo.js interactive

# 运行 validator 套件（需要 dist/ 已构建）
npm run validator

# 运行单个 validator spec
cargo run --package validator -- validator-specs/smoke_cancel.toml --out validator-output
```

## 关键架构细节

### Workspace 布局

- 根目录 `Cargo.toml` 是一个纯 workspace manifest（无 `[package]`），声明 `members = ["crates/*"]`。
- `crates/napi-bindings/Cargo.toml` 定义了 `electron-rust-screenshot` 包，使用 `crate-type = ["cdylib", "rlib"]`。
- npm build script 使用 `--cargo-cwd crates/napi-bindings`，使 `napi-rs` 能在 bindings crate 中编译，同时 JS 产物输出到 `dist/`。

### 阻塞式同步 `start()` API

`crates/napi-bindings/src/lib.rs` 暴露了一个 `#[napi] pub fn start(config: Option<ScreenshotConfig>) -> Result<String>`。**该函数会阻塞调用线程**，因为 macOS 要求 `winit::EventLoop` 在主线程运行。它只有在 overlay 关闭后才返回，返回一个 JSON 字符串表示最终事件（`saved`、`cancelled` 或 `error`）。

由于 `start()` 阻塞，E2E runner（`e2e/helpers/runner.ts`）将其放入 `child_process` 中启动，轮询结果 JSON 临时文件，然后杀掉进程。

### JS 入口稳定性

`npm run build` 会自动生成 `dist/index.js` 和 `dist/index.d.ts`。稳定的入口封装是 `dist/lib.js`，它引入 `./index.js` 并解析 JSON 结果。`package.json` 指定 `"main": "dist/lib.js"` 和 `"types": "dist/index.d.ts"`。请勿直接编辑 `dist/index.js` —— 每次 build 都会被覆盖。

### 核心层（`crates/screenshot-core/src/`）

- **`lib.rs`** — re-export `core`、`overlay` 和 `platform`。
- **`core/`** — 与平台无关的业务逻辑：
  - `engine.rs` — 状态机（`Idle -> Capturing -> OverlayRunning -> FreeSelecting -> Editing -> Saving`）。
  - `capture.rs` — `PlatformCapture` trait 和 `MockCapture`。
  - `editor.rs` — `EditorState`，包含 tool、layer、undo/redo。
  - `events.rs` — `EngineEvent` enum 和 `EventBus`（基于 crossbeam-channel）。`EngineEvent::Metrics(MetricsPayload)` 是 internally-tagged enum（`#[serde(tag="type")]`）中的 newtype variant，serde 通常不支持此组合，但实测可正常序列化/反序列化。
  - `types.rs` — 逻辑坐标（`Rect`、`LogicalPoint`）、`Color`、`ScreenInfo`、`DetectedWindow`。
  - `dpi.rs` — 逻辑坐标与物理像素之间的转换。
  - `window.rs` — `WindowDetector`，基于 `rstar::RTree` 做窗口 hover 命中检测。
  - `perf.rs` — `PerformanceMonitor`，记录 hit-test 耗时并通过 `MetricsPayload` 上报。
- **`harness/`** — 测试 harness：
  - `protocol.rs` 在 `crates/harness-protocol/` 中 — `Command` / `ServerMessage` JSONL 协议。
  - `transport.rs` / `server.rs` — 本地 socket 连接、命令队列、状态快照推送。
  - `dispatch.rs` — 每帧 tick，将 inbound `Command` 分发给 `injector_scripted` 或 `injector_real`。
  - `injector_scripted.rs` — 直接调用 engine 方法（绕开 UI 事件队列）。
  - `injector_real/` — macOS 通过 `cliclick` 注入系统事件；非 macOS 为返回 `Unsupported` 的 stub。
  - `coords.rs` — `logical_to_physical` 转换，用于 real-tier 坐标。
  - `modifiers.rs` — 修饰键解析与平台键名映射。
- **`overlay/`** — UI 层：
  - `manager.rs` — `OverlayManager` / `MultiWindowApp`（winit `ApplicationHandler`）。为每个物理 display 创建独立的窗口，各自持有独立的 `egui::Context`、`EguiState` 和 `egui_glow::Painter`，以避免 GL context 交叉污染。
  - `app.rs` — `ScreenshotApp`（egui app）。每个窗口接收一个 `global_offset`，因此所有显示器都能渲染完整的 frame 集合以支持跨屏拖拽。
  - `gl.rs` — `GlContext` 辅助模块，使用 `glutin 0.32` + `glutin-winit 0.5`。
  - `save.rs` — `composite_image` 负责从所有相交屏幕拼接 captured frame，对 DPI 不一致的部分使用 Lanczos3 重采样，并对裁剪后的输出应用 mosaic。
  - `toolbar.rs` — `draw_toolbar`，负责 editor 模式下的工具栏 UI。
- **`platform/`** — 平台抽象层：
  - `traits.rs` — 定义 `PlatformCapture`、`PlatformClipboard`、`PlatformOverlay`、`PlatformWindowEnumerator` trait，以及 `create_capture()` 工厂函数和 `Backend` 类型别名。
  - `macos/` — macOS 专属实现：
    - `capture_cg.rs` — 基于 `CGDisplayCreateImage` 的 capture。对 retina/rotated display 使用 `CGImage::bytes_per_row()` 读取数据。
    - `capture_sck.rs` — `ScreenCaptureKit` 封装（当前内部 fallback 到 `MacOsCgCapture`）。
    - `window.rs` — 基于 `CGWindowListCopyWindowInfo` 的窗口枚举。
    - `clipboard.rs` — `NSPasteboard` 图片复制。
    - `overlay_ext.rs` — `setup_window` 和 `set_mouse_passthrough` 的 objc 实现。
    - `mod.rs` — `MacosBackend`，实现所有 platform traits。
  - `windows/` — Windows 平台 stub（`WindowsBackend` 待实现）。
  - `unsupported.rs` — 非 macOS / Windows 平台的 `UnsupportedBackend` fallback（仅返回错误）。
- **`bridge/`**（位于 `crates/napi-bindings/src/`）— napi bindings：
  - `config.rs` — `ScreenshotConfig` napi struct。
  - `session.rs` — `ScreenshotSession` napi class。
  - `events_js.rs` — `EngineEvent` 的手动 JSON 序列化。

### 多显示器架构

`MultiWindowApp` 在单个 `winit::EventLoop` 中驱动多个 `WindowState`。关键设计点：

1. **每窗口 GL 隔离** — 每个 display 拥有独立的 `GlContext`、`egui::Context` 和 `Painter`。每次 `RedrawRequested` 前先调用 `make_current()`。`WindowState` 实现 `Drop`，在当前正确的 context 上调用 `painter.destroy()`。
2. **跨显示器拖拽** — 所有窗口共享同一个 `Arc<Mutex<Engine>>`。窗口本地坐标先转换为全局逻辑坐标，再传入 engine 的鼠标事件处理函数。
3. **编辑态交互锁定** — 选区确认后 engine 进入 `Editing` 状态，`update_interactivity()` 计算哪些窗口与选区相交，并通过 `platform::Backend::set_mouse_passthrough` 对剩余显示器调用忽略鼠标事件。toolbar 只在包含选区中心的那台显示器上显示。
4. **保存/合成** — `composite_image` 将选区与所有 screen frame 做 intersect，计算 union 输出图像（以 `dominant_dpi` 为基准），然后将每块裁剪区域叠加到最终图像上。
5. **纹理尺寸上限** — `OverlayManager` 创建窗口时查询 OpenGL `MAX_TEXTURE_SIZE` 并传给 `egui_winit::State`，避免 egui 默认的 2048 限制导致 retina 截图纹理加载失败。

### 坐标系

`core/` 内的所有几何数据均采用 **logical coordinates**。仅在 capture 时（`CGDisplayCreateImage`）和保存时（`dpi.rs`）才做物理像素转换。每个 overlay window 均按对应 display 的 logical bounds 设定大小。

### Harness / Validator 自动化

`crates/validator/` 是一个 Rust 二进制 crate，通过 **JSONL socket 协议** 直接驱动截图引擎，支持 `scripted`（直接调用 engine 方法）和 `real`（通过 `cliclick` 注入系统事件）两种 tier。

常用命令：

```bash
# 运行所有 validator specs（需要 dist/ 已构建）
npm run validator

# 仅构建 validator
npm run validator:build

# 运行单个 spec
cargo run --package validator -- validator-specs/smoke_cancel.toml --out validator-output

# 运行端到端 smoke test（默认 #[ignore]，因为需要 dist/ 构建产物和 GUI 环境）
cargo test --package validator --test smoke -- --ignored
```

`validator-specs/` 目录包含 TOML 格式的测试场景：

- `smoke_cancel.toml` — 启动 overlay 后按 Escape 取消
- `hybrid_save.toml` — scripted drag + real Enter 保存
- `e2e_cancel.toml` / `e2e_free_select.toml` / `e2e_resize.toml` / `e2e_move.toml` — 对应原有 E2E 场景
- `asserts_demo.toml` — 覆盖所有断言类型（event_emitted、artifact_exists、artifact_dimensions、performance 等）

spec 支持 `setup`（save_path、format、quality）、`steps`（drag、key_press、wait_for、sleep 等）和 `asserts`（事件断言、文件断言、性能断言）。详细格式参考 `crates/validator/src/spec.rs`。

### E2E 自动化（原有 Jest + AppleScript）

E2E 套件（`e2e/`）通过 AppleScript 调用 `cliclick` 模拟鼠标/键盘。关键 helper：

- `e2e/helpers/runner.ts` — 在 child process 中启动 `start()`，并通过 `SCREENSHOT_TEST_TIMEOUT_MS` 控制超时。
- `e2e/helpers/mouse.ts` / `keyboard.ts` — 封装 AppleScript / `cliclick` 的鼠标与键盘操作。
- `e2e/helpers/window.ts` — 通过 AppleScript 打开/关闭 Safari fixture 窗口。

`cliclick` 注入事件需要 **Accessibility permissions**。若权限不可用，可设置 `SCREENSHOT_TEST_MOCK_DRAG=x1,y1,x2,y2`（如 CI 中使用的 `300,300,500,500`），overlay 在启动后约 5 秒内部模拟该坐标的拖拽 + 保存。

### macOS Overlay 窗口行为

`MultiWindowApp::resumed` 创建窗口时不使用 `Fullscreen::Borderless(None)`（以避免 macOS Space 切换问题），而是直接用各 display 的 logical bounds 设置 `inner_size` 和 `position`。窗口创建后通过 `platform::macos::overlay_ext::setup_window` 执行：

1. `NSApplication activateIgnoringOtherApps:YES`
2. `setLevel:NSStatusWindowLevel` (25)
3. `makeKeyAndOrderFront:`

这确保 overlay 能接收键盘事件（Esc、Enter）并层级高于其他所有窗口。

## 编码工作流规范

进行任何代码修改时，必须借助日志、AppleScript（或其他平台的对应工具链）完成对修改效果的观测验证。验证范围应覆盖 golden path 与边界 case：

- **macOS 平台**：优先利用 `cargo test --workspace` 与 `npm test` 做快速验证；涉及多显示器、窗口层级或鼠标交互的改动，应运行 E2E 或手动触发 overlay，通过终端日志与 AppleScript / `cliclick` 注入事件来确认行为。
- **其他平台**：使用对应平台的事件注入与窗口管理工具（如 Linux 的 `xdotool`、Windows 的 AutoHotkey / UI Automation）进行等效验证。
- **验证不可跳过**：不得以"代码逻辑很简单"为由跳过实际运行验证。尤其是涉及 GL、`winit` 窗口、`objc` unsafe block 或 composite 算法改动时，必须在修改后立即 build 并观测。

## 编码规范

- **代码质量**：坚持清晰的错误传播（优先 `Result` 而非 `unwrap`/`expect`），避免在 GUI 初始化路径中吞掉错误；unsafe block 需最小化并封装成可审查的边界层。
- **职责明确**：`screenshot-core` 中不应出现 `napi` 依赖或 Node 相关逻辑；`napi-bindings` 层只做数据转换与 API 暴露。overlay 只负责渲染与事件分发，合成算法放在 `save.rs`，状态机放在 `engine.rs`。
- **可测试**：核心业务逻辑（engine、editor、composite、geometry）必须能在不启动窗口系统的情况下通过单元/集成测试覆盖。新增功能时同步添加测试；mock 实现参考 `capture.rs` 中的 `MockCapture`。
