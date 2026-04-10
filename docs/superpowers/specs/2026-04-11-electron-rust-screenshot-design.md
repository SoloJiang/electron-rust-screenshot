# Electron + Rust 原生截图工具设计文档

**日期**: 2026-04-11  
**范围**: 完整生产级方案（macOS 为主验证，Windows 同步预留）  
**目标体验**: 微信截图级别（框选、窗口悬浮识别、原生编辑、多显示器、高性能）

---

## 1. 项目背景与目标

通过 `napi-rs` 将 Rust 原生截图能力桥接到 Electron/Node.js 应用中。Node.js 侧仅通过简单的 `start()` 调用即可启动截图流程；Rust 侧负责全部原生交互（overlay、窗口检测、编辑、保存），以提供最低延迟、最丝滑的系统级体验。

### 核心成功指标

- **交互体验**: 启动后 < 150ms 进入可操作状态；鼠标移动 → 窗口高亮 < 16ms。
- **功能完整**: 支持框选截图、悬浮窗口自动识别、马赛克、矩形/椭圆/箭头/涂鸦/文字标注、撤销重做。
- **性能**: 零拷贝截屏（macOS ScreenCaptureKit / Windows DXGI Desktop Duplication）；静止时 overlay CPU/GPU 占用趋近于零。
- **可观测**: AppleScript 驱动的 E2E 自动化测试 + 实时性能埋点（`metrics` 事件）。

---

## 2. 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│  Electron / Node.js                                         │
│  - screenshot.start(config) 返回 EventEmitter               │
│  - 监听 started / windowHovered / regionSelected / saved    │
│     / cancelled / error / metrics 事件                      │
└─────────────────────────────────────────────────────────────┘
                              ↑ napi-rs 绑定
┌─────────────────────────────────────────────────────────────┐
│  napi-rs (Rust Bridge)                                      │
│  - 提供 sync 的 start() -> JsObject                         │
│  - 通过 napi_threadsafe_function 将 Rust 事件异步回调给 JS  │
│  - 配置序列化/反序列化                                       │
└─────────────────────────────────────────────────────────────┘
                              ↑ 内部 API
┌─────────────────────────────────────────────────────────────┐
│  Rust Screenshot Core                                       │
│  - Engine: 生命周期总控（启动/取消/完成）                    │
│  - OverlayManager: 每台显示器一个 overlay 窗口               │
│  - PlatformCapture: 统一截屏接口，屏蔽平台差异               │
│  - WindowDetector: R-tree 加速的实时窗口命中检测             │
│  - EditorState: 选区、图层、撤销栈、工具状态                 │
│  - EventBus: 异步事件广播给 Bridge 层                       │
│  - PerformanceMonitor: 帧时间/内存/命中测试延迟埋点          │
└─────────────────────────────────────────────────────────────┘
                              ↑ 平台适配
┌─────────────────────────────────────────────────────────────┐
│  Platform Adapters                                          │
│  macOS:                                                     │
│    - ScreenCaptureKit (12.3+) / CGDisplay (fallback) 截屏   │
│    - CGWindowList + AXUIElement 枚举窗口                    │
│    - winit + egui_glow (Metal/ OpenGL) overlay              │
│    - AppleScript E2E helpers                                │
│  Windows:                                                   │
│    - DXGI Desktop Duplication (primary) / BitBlt (fallback) │
│    - EnumWindows + DWM / UI Automation 枚举窗口             │
│    - winit + egui_glow (DirectX/OpenGL) overlay             │
│    - PowerShell/C# E2E helpers (预留)                       │
└─────────────────────────────────────────────────────────────┘
```

### 架构原则

1. **Rust 原生全包 UI**: Electron 不做任何 overlay 绘制，只接收事件和最终产物。
2. **零拷贝优先**: 截屏结果尽量作为 GPU 纹理直接供给 egui 渲染，避免 CPU 往返拷贝。
3. **按需重绘**: overlay 没有视觉变化时不请求任何 `RedrawEventsCleared`，静止时零开销。
4. **逻辑坐标统一**: 窗口边界、鼠标坐标、选区矩形在 Rust 内部全部使用逻辑坐标，仅在保存落盘时转回物理像素。

---

## 3. Rust Core 组件职责

### 3.1 DpiAwareness

- 负责 `logical_px <-> physical_px` 的双向转换。
- 每台显示器拥有一个 `DpiScale`（macOS: `backingScaleFactor`，Windows: `GetDpiForMonitor / 96.0`）。
- 所有几何计算（鼠标位置、窗口 bounds、选区 rect）在 Core 层流通时强制使用逻辑坐标。

### 3.2 PlatformCapture

统一截屏的 trait 接口：

```rust
trait PlatformCapture {
    fn capture_all_screens(&self) -> Result<Vec<ScreenFrame>, CaptureError>;
}

struct ScreenFrame {
    id: ScreenId,
    logical_bounds: Rect,
    dpi_scale: f64,
    // 平台相关：macOS 为 IOSurface/Metal 纹理；Windows 为 D3D11 纹理
    texture: PlatformTexture,
}
```

- **macOS 主方案**: `ScreenCaptureKit` 的 `SCContentFilter` 和 `SCStream`，输出到 `IOSurfaceRef`，直接作为 `egui::TextureId` 的 Metal backend 数据源。
- **macOS 回退**: `CGDisplayCreateImage` -> 提取 RGBA bytes -> 上传 GPU texture（老系统兼容）。
- **Windows 主方案**: `DXGI Desktop Duplication` 获取 `ID3D11Texture2D`，通过共享 handle 给 egui 的 DirectX render context。
- **Windows 回退**: `BitBlt` / `PrintWindow` -> CPU RGBA buffer -> GPU upload。

### 3.3 WindowDetector

- **macOS 数据源**: `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID)` 做初筛，再用 `AXUIElement` 过滤不可见的系统窗口。
- **Windows 数据源**: `EnumWindows` 获取顶层窗口句柄，通过 `DwmGetWindowAttribute(DWMWA_CLOAKED)`、`IsWindowVisible` 过滤。
- **空间索引**: 窗口列表刷新时（默认 1 秒/次）重建 `rstar::RTree`。鼠标移动时做 R-tree 查询，
  将 `O(n)` 线性扫描降到 `O(log n)`。
- **输出**: `DetectedWindow { id, title, bounds: logical Rect, owner_pid, z_order }`。

### 3.4 OverlayWindow（每台显示器一个）

- `winit::WindowBuilder` 参数:
  - `with_transparent(true)`
  - `with_decorations(false)`
  - `with_always_on_top(true)`
  - `with_fullscreen(Some(Fullscreen::Borderless(None)))`
- 使用 `egui_glow`（或平台更快的 `egui-wgpu`/custom）做渲染上下文。
- 每帧绘制顺序:
  1. 截屏底图（GPU texture 全屏贴图）
  2. 窗口高亮边框（悬浮模式下）
  3. 自由框选矩形/十字虚线（拖动模式下）
  4. 编辑遮罩（选区外 40% 黑半透明）
  5. 编辑图层（马赛克、矩形、椭圆、箭头、涂鸦、文字）
  6. 工具栏/取色器 HUD
  7. 性能调试 HUD（可选）

### 3.5 EditorState

```rust
struct EditorState {
    selection: Option<Rect>,
    layers: Vec<Layer>,
    undo_stack: Vec<LayerOp>,
    redo_stack: Vec<LayerOp>,
    active_tool: Tool,
    tool_color: Color32,
    tool_size: f32,
    // 当前正在绘制但尚未落笔的预览（如拖动中的矩形）
    preview: Option<Layer>,
}
```

**Tool 枚举**:
- `Rect`, `Ellipse`, `Arrow`, `Brush`, `Mosaic`, `Text`

**Layer 枚举**:
- `ShapeRect { rect, stroke_width, color }`
- `ShapeEllipse { rect, stroke_width, color }`
- `Arrow { start, end, stroke_width, color }`
- `BrushPath { points, stroke_width, color }`
- `MosaicPath { points, block_size }`（底层通过采样原图像素绘制方块）
- `Text { pos, text, font_size, color }`（egui 的 `TextShape`）

**撤销/重做**:
- 每次落笔生成一个 `LayerOp::AddLayer { id }`，压入 `undo_stack`。
- 撤销时从 `layers` 移除对应 id，压入 `redo_stack`。
- 新建操作会清空 `redo_stack`。

### 3.6 Engine（生命周期总控）

```
Idle
  ↓ start()
Capturing ───→ Error
  ↓ 所有屏幕截完
OverlayRunning
  ├── ESC / 右键 / 双击空白处 ──→ Cancelled
  ├── 鼠标移动 ──→ WindowHovered event（R-tree 查询）
  ├── MouseDown + 立刻拖动 ──→ FreeSelecting
  │      └── MouseUp ──→ Editing（有效选区）或 OverlayRunning（无效选区）
  └── MouseDown + 无拖动松开 ──→ 当前悬浮窗口作为选区 ──→ Editing
Editing
  ├── ESC ──→ 返回 OverlayRunning（清空编辑图层，保留选区）
  ├── 工具栏操作 ──→ 修改 EditorState → 标记重绘
  ├── Ctrl+Z / Ctrl+Shift+Z ──→ Undo / Redo
  └── Enter / Ctrl+C / 点击保存 ──→ Saving → Saved event → 退出
```

**反抖动规则**:
- `MouseDown` 后 50ms 内即使移动也不判定为拖动（避免手抖误切）。
- 点击判定：MouseDown 到 MouseUp，位移 < 4 logical px 且时间 < 250ms。

### 3.7 PerformanceMonitor

- `capture_ms`: 截屏总耗时。
- `window_enum_ms`: 窗口枚举耗时。
- `hit_test_p99_ms`: 最近 16 次 R-tree 命中测试的 P99 耗时。
- `frame_time_ms`: 上一帧渲染耗时（`RedrawRequested` 开始到 present 完成）。
- `egui_paint_ms`: `egui::Context::run` 内部耗时。
- `memory_mb`: 当前进程 RSS（`sysinfo` 每秒读取）。

上报策略:
- 通过 bounded channel 向 EventBus 发送，JS 侧 `metrics` 事件默认 500ms throttle。
- `showDebugHud: true` 时，Overlay 左上角绘制实时面板。

---

## 4. Node.js API 设计

### 4.1 入口与配置

```ts
// native.d.ts
export interface ScreenshotNative {
  start(config: ScreenshotConfig): ScreenshotSession;
}

export interface ScreenshotConfig {
  /** 本地保存目录，默认系统临时目录 */
  savePath?: string;
  /** 图片格式，默认 'png' */
  format?: 'png' | 'jpg' | 'webp';
  /** 压缩质量 0-100，默认 90（仅 jpg/webp） */
  quality?: number;
  /** 马赛克块大小，默认 8 logical px */
  mosaicBlockSize?: number;
  /** 默认工具颜色，默认 '#ff0000' */
  defaultColor?: string;
  /** 默认画笔粗细，默认 3 logical px */
  defaultSize?: number;
  /** 语言，默认 'zh-CN' */
  locale?: 'zh-CN' | 'en';
  /** 调试：左上角显示性能 HUD */
  showDebugHud?: boolean;
  /** metrics 事件上报间隔 ms，默认 500 */
  metricsIntervalMs?: number;
}
```

### 4.2 Session EventEmitter

```ts
export interface ScreenshotSession {
  on(event: 'started', listener: (payload: StartedPayload) => void): this;
  on(event: 'windowHovered', listener: (payload: WindowHoveredPayload) => void): this;
  on(event: 'regionSelected', listener: (payload: RegionSelectedPayload) => void): this;
  on(event: 'saved', listener: (payload: SavedPayload) => void): this;
  on(event: 'cancelled', listener: () => void): this;
  on(event: 'error', listener: (payload: ErrorPayload) => void): this;
  on(event: 'metrics', listener: (payload: MetricsPayload) => void): this;

  /** 程序性取消（供 Electron 用户点击外部按钮时调用） */
  cancel(): void;
}

export interface StartedPayload {
  screens: Array<{
    id: string;
    name: string;
    logicalBounds: { x: number; y: number; w: number; h: number };
    dpiScale: number;
    captureSuccess: boolean;
  }>;
}

export interface WindowHoveredPayload {
  window: {
    id: string;
    title: string;
    bounds: { x: number; y: number; w: number; h: number };
  };
}

export interface RegionSelectedPayload {
  screenId: string;
  rect: { x: number; y: number; w: number; h: number };
}

export interface SavedPayload {
  /** 最终保存的绝对路径 */
  path: string;
  /** 是否同时复制到系统剪贴板 */
  copied: boolean;
}

export interface ErrorPayload {
  code: 'CAPTURE_FAILED' | 'NO_DISPLAY' | 'PERMISSION_DENIED' | 'SAVE_FAILED' | 'UNKNOWN';
  message: string;
}

export interface MetricsPayload {
  captureMs: number;
  windowEnumMs: number;
  hitTestP99Ms: number;
  frameTimeMs: number;
  eguiPaintMs: number;
  memoryMb: number;
}
```

### 4.3 快捷键（overlay 激活时全局生效）

| 按键 | 行为 |
|------|------|
| `Esc` | 取消 / 返回上一步 |
| `Enter` / `Return` | 保存并退出 |
| `Ctrl+C` / `Cmd+C` | 复制最终图像到系统剪贴板并退出 |
| `Ctrl+Z` / `Cmd+Z` | 撤销 |
| `Ctrl+Shift+Z` / `Cmd+Shift+Z` | 重做 |
| `1`~`6` | 快速切换颜色 |
| `R` / `E` / `A` / `B` / `M` / `T` | 快速切换工具（Rect/Ellipse/Arrow/Brush/Mosaic/Text） |

---

## 5. E2E 测试架构

### 5.1 目录结构

```
e2e/
├── fixtures/
│   ├── sample-window.html       # 固定尺寸、已知颜色的测试窗口
│   └── reference-images/        # 基准截图
├── helpers/
│   ├── platform.ts              # 按 process.platform 分发 AppleScript / PowerShell
│   ├── applescript.ts           # 运行 .scpt 脚本封装
│   ├── mouse.ts                 # moveTo / drag / click
│   ├── keyboard.ts              // keypress 封装
│   ├── window.ts                # 打开/定位/关闭 fixture 窗口
│   └── screen.ts                # 获取主屏幕尺寸、DPI
├── specs/
│   ├── capture-fullscreen.spec.ts
│   ├── window-hover.spec.ts
│   ├── free-select.spec.ts
│   ├── edit-tools.spec.ts
│   ├── cancel-flow.spec.ts
│   └── performance.spec.ts
└── scripts/
    ├── open_fixture_window.scpt
    ├── move_mouse.scpt
    ├── click.scpt
    ├── drag.scpt
    ├── keypress.scpt
    └── get_mouse_position.scpt
```

### 5.2 AppleScript 测试示例

```ts
// e2e/specs/window-hover.spec.ts
test('hover over a known window triggers windowHovered', async () => {
  await WindowHelper.openFixture({ x: 200, y: 200, w: 400, h: 300, color: '#ff0000' });

  const session = screenshot.start({ savePath: tmpDir });
  const events: any[] = [];
  session.on('windowHovered', e => events.push(e));
  session.on('regionSelected', e => events.push(e));

  await sleep(800);                       // 等待 overlay 就绪
  await Mouse.moveTo(400, 350);           // 移到 fixture 窗口中心
  await sleep(200);
  await Mouse.click();
  await sleep(200);

  const hovered = events.find(e => e.window);
  expect(hovered.window.bounds).toMatchObject({ x: 200, y: 200, w: 400, h: 300 });

  session.cancel();
  await WindowHelper.closeAllFixtures();
});
```

### 5.3 性能回归测试

```ts
test('frame time and hit-test latency under 16ms', async () => {
  const session = screenshot.start({ savePath: tmpDir });
  const metrics: MetricsPayload[] = [];
  session.on('metrics', m => metrics.push(m));

  await sleep(500);
  await Mouse.moveTo(400, 350);
  await sleep(500);

  const bad = metrics.filter(
    m => m.frameTimeMs > 16 || m.hitTestP99Ms > 16
  );
  expect(bad.length).toBeLessThan(metrics.length * 0.05);
  session.cancel();
});
```

### 5.4 Windows 预留

`e2e/helpers/platform.ts` 预留 Windows 分支：未来通过 PowerShell / C# 脚本（`System.Windows.Forms.Cursor.Position`、`SendKeys`）实现与 AppleScript 完全对齐的 API。

---

## 6. 错误处理

所有 Rust 错误统一收敛到 `ErrorPayload`，通过 EventBus 发给 JS。不会 panic 到 Node.js 进程。

- **`CAPTURE_FAILED`**: 平台截屏 API 调用失败。
- **`PERMISSION_DENIED`**: macOS 未授予屏幕录制权限（需要 `CGRequestScreenCaptureAccess()` 或 Info.plist 权限描述）。
- **`NO_DISPLAY`**: 无法枚举到任何可用显示器。
- **`SAVE_FAILED`**: 最终图像写入磁盘失败。
- **`UNKNOWN`**: 未分类异常。

Node.js 侧 `start()` 是 sync 返回 `ScreenshotSession`，真正的错误监听在异步 `error` 事件里。

---

## 7. 跨平台策略

| 模块 | macOS | Windows |
|------|-------|---------|
| 截屏 | ScreenCaptureKit (12.3+) / CGDisplay fallback | DXGI Desktop Duplication / BitBlt fallback |
| 窗口枚举 | CGWindowList + AXUIElement | EnumWindows + DWM UI Automation |
| Overlay | winit + egui_glow (Metal/ OpenGL) | winit + egui_glow (DirectX/ OpenGL) |
| E2E | AppleScript | PowerShell / C# (预留) |
| 剪贴板 | `NSPasteboard` | `OpenClipboard` / `SetClipboardData` |

### 开发顺序

1. **Phase 1**: macOS 完整功能跑通（截屏、overlay、编辑、保存、E2E）。
2. **Phase 2**: Windows `PlatformCapture` + `WindowDetector` 替换实现，UI 层（egui）复用。
3. **Phase 3**: Windows E2E 补齐 + 跨平台 CI 集成。

---

## 8. 性能硬指标

| 指标 | 目标值 |
|------|--------|
| 所有屏幕截屏完成 | < 100 ms |
| overlay 全屏覆盖就绪 | < 50 ms（截屏完成后） |
| 鼠标移动 → 窗口高亮 | < 16 ms（60fps 标准） |
| 框选结束 → 编辑模式切换 | < 50 ms |
| 编辑保存 → 文件落盘 | < 200 ms |
| 静止状态 CPU/GPU 占用 | 趋近于 0（无重绘请求） |
| 截屏数据拷贝次数 | 理想情况下 0 次（GPU texture 直通） |

---

## 9. 风险与约束

1. **macOS 权限**: 用户首次使用必须授予屏幕录制权限，需要在 Electron 应用层做引导 UI。
2. **Metal/OpenGL 上下文**: egui_glow 在不同 macOS 版本和显卡上的驱动兼容性需要充分测试。
3. **Windows DXGI**: 某些远程桌面或虚拟机环境不支持 Desktop Duplication，必须有 `BitBlt` 回退。
4. **egui 的美观度**: 默认风格偏工具向，需要投入时间做圆角、阴影、动画、图标皮肤定制。
5. **剪贴板**: macOS `NSPasteboard` 写入图片后，Electron/Node.js 侧不能二次读取确认，E2E 测试需要以 `saved` 事件 + `copied: true` 做间接断言。
