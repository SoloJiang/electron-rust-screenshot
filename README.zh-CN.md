# electron-rust-screenshot

基于 [napi-rs](https://napi.rs/) 构建的 Rust + Node.js 截图工具，提供 macOS 原生交互体验。

[English](./README.md) · [中文](./README.zh-CN.md)

## 特性

- 全屏 overlay，支持多显示器，每个 display 拥有独立的 GL context
- 跨显示器拖拽选区，几何运算全程使用逻辑坐标
- 窗口悬停高亮（基于 R-tree 空间索引）+ 双击直接抓取窗口
- 编辑器内嵌：矩形 / 椭圆 / 箭头 / 笔刷 / 马赛克 / 文字，支持 undo / redo
- 马赛克脱敏、保存到磁盘、一键复制到剪贴板
- 阻塞式同步 `start()` API，专为 Electron 主进程设计

## 平台支持

| 平台 | 状态 |
|------|------|
| macOS    | 完整支持（CoreGraphics 截图，NSStatusWindowLevel overlay） |
| Windows  | 仅占位（`WindowsBackend` 待实现） |
| Linux 等 | `UnsupportedBackend`，调用直接返回错误 |

## 安装

```bash
npm install electron-rust-screenshot
```

> 包内附带 `napi-rs` 编译产物。如需源码构建，需要 stable Rust 工具链以及 Node.js 20+。

## 源码构建

```bash
git clone <本仓库>
cd electron-rust-screenshot
npm install
npm run build        # 产物输出到 dist/*.node 和 dist/index.js
```

## 快速开始

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

> **注意：** `start()` 会阻塞调用线程直到 overlay 关闭，因为 macOS 要求 `winit::EventLoop` 运行在主线程。在 Electron 中应在 main process 调用。

## 运行 Demo

```bash
node scripts/demo.js interactive   # png + 红色画笔
node scripts/demo.js jpg           # jpeg + 绿色画笔
node scripts/demo.js clipboard     # png + 蓝色画笔
```

## 快捷键

| 操作 | 按键 |
|------|------|
| 选区 / 绘制图形 | 鼠标拖拽 |
| 高亮悬停窗口 | 鼠标移动 |
| 直接抓取悬停窗口 | 鼠标双击 |
| 切换工具(矩形 / 椭圆 / 箭头 / 笔刷 / 马赛克 / 文字) | `1`–`6` |
| 撤销 | `Cmd+Z` |
| 重做 | `Cmd+Shift+Z` |
| 复制到剪贴板（编辑模式下） | `C` |
| 保存 | `Enter` |
| 取消 | `Esc` |

## API

### `start(config?: ScreenshotConfig): string`

同步启动 overlay。返回描述最终事件的 JSON 字符串。

### `ScreenshotConfig`

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `savePath` | `string` | `/tmp` | 截图保存路径 |
| `format` | `'png' \| 'jpg'` | `'png'` | 编码格式 |
| `quality` | `number`（1–100） | `90` | JPEG 质量（PNG 忽略） |
| `mosaicBlockSize` | `number` | `8` | 马赛克块大小（逻辑像素） |
| `defaultColor` | `string`（hex） | `#ff0000` | 默认笔刷 / 图形颜色 |
| `defaultSize` | `number` | `3` | 默认笔刷 / 描边宽度 |
| `locale` | `string` | `zh-CN` | UI 语言 |
| `showDebugHud` | `boolean` | `false` | 是否显示性能 HUD |
| `metricsIntervalMs` | `number` | `500` | `metrics` 事件触发间隔 |

### 最终事件 JSON

```jsonc
{ "type": "saved",     "path": "/tmp/shot.png", "copied": false }
{ "type": "cancelled" }
{ "type": "error",     "code": "<error-code>",  "message": "<reason>" }
```

## 项目结构

```
crates/
  screenshot-core/    # 平台无关的 Rust 库（rlib）
    src/core/         # engine、capture、editor、events、types、dpi、perf、window detector
    src/overlay/      # winit + egui 多窗口 overlay、GL 辅助、合成、工具栏
    src/platform/     # macOS / Windows / unsupported backend
  napi-bindings/      # 通过 napi-rs 暴露的 cdylib
    src/bridge/       # ScreenshotConfig、ScreenshotSession、事件 JSON 序列化
dist/                 # 构建产物（.node + JS wrapper）
e2e/                  # Jest + AppleScript E2E 测试
scripts/              # demo.js（交互式）与 test-smoke.js（冒烟）
```

更深入的架构细节（多显示器 GL 隔离、坐标系、NSWindow 设置、合成算法）见 [`CLAUDE.md`](./CLAUDE.md)。

## 开发与测试

```bash
cargo test --workspace                                  # Rust 单元 + 集成测试
npm test                                                # Node 冒烟测试
cd e2e && npx jest --runInBand                          # AppleScript E2E 套件
cargo fmt --all -- --check                              # CI lint gate
cargo clippy --workspace --all-targets -- -D warnings   # CI lint gate
```

E2E 测试通过 [`cliclick`](https://github.com/BlueM/cliclick) 注入鼠标事件，需要为其授予 Accessibility 权限。在 headless / 沙盒环境下，可设置 `SCREENSHOT_TEST_MOCK_DRAG=x1,y1,x2,y2`（例如 `300,300,500,500`），overlay 会在启动约 5 秒后内部模拟该坐标的拖拽。

## License

ISC
