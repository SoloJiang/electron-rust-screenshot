# 可拖动/缩放选中框设计文档

## 1. 概述

在截图工具的 `Editing`（编辑）状态下，用户绘制标注前或标注后，应该能够拖动已确认的选区框来改变位置，或拖拽边框/四角手柄来缩放选区。所有已有的标注图层会跟随选区一起移动/缩放。

## 2. 目标

- 在 `Editing` 状态下，支持对 `editor.selection` 进行**拖动平移**和**8 向缩放**。
- 已有标注图层（`editor.layers`）随选区同步变换。
- 变换操作可撤销/重做（单次拖拽结束只产生一条 undo 记录）。
- 光标悬停在边框/角点时有视觉反馈（光标样式变化）。

## 3. 非目标

- 在 `FreeSelecting`（自由拖拽创建选区）阶段支持移动/缩放（本次不做）。
- 旋转选区。
- 对单个图层做独立的 resize/transform。
- 非等比缩放下的特殊处理（如保持箭头比例等复杂几何约束）。

## 4. 架构与职责分层

采用“**Overlay 负责命中测试与光标反馈，Editor 负责纯几何变换，Engine 负责状态同步与 Undo**”的分层方案：

| 层级 | 负责内容 |
|------|----------|
| `overlay/app.rs` | 在 `Editing` 分支内做手柄命中测试；设置光标样式；根据命中结果把鼠标事件路由给 engine 的 transform 接口 |
| `core/types.rs` | 提供 `Rect::hit_test_resize_handle(...)` 纯函数，用于判断点落在哪个交互区域 |
| `core/editor.rs` | 提供纯计算方法：`transform_selection(start_rect, delta, kind) -> (Rect, Vec<Layer>)`，把原始 layers 按新选区做平移/缩放 |
| `core/engine.rs` | 维护 `selection_transform: Option<SelectionTransformState>`；处理 `on_selection_transform_start/drag/end`；在拖拽结束时把变更打包为 `LayerOp::UpdateSelectionAndLayers` 压入 undo 栈 |

## 5. 详细设计

### 5.1 命中区域（Hit Test）

对于选区 `Rect`，手柄尺寸 `HANDLE_SIZE = 8.0`（逻辑坐标像素）：

- **8 个角/边**：西北(NW)、北(N)、东北(NE)、东(E)、东南(SE)、南(S)、西南(SW)、西(W)。
- **优先级**：角 > 边 > 内部平移区 > 外部无交互。
- **平移区**：选区内部（距离四边各大于 `HANDLE_SIZE/2` 的区域）用于整体拖动。

新增类型：

```rust
pub enum ResizeHit {
    Move,
    ResizeEdge { edge: Edge },   // N, S, E, W
    ResizeCorner { corner: Corner }, // NW, NE, SW, SE
}

impl Rect {
    pub fn hit_test_resize_handle(&self, p: LogicalPoint, handle_size: f64) -> Option<ResizeHit>;
}
```

### 5.2 拖动状态（Engine）

在 `Engine` 中新增：

```rust
pub struct SelectionTransformState {
    pub kind: ResizeHit,
    pub start_pointer: LogicalPoint,
    pub original_selection: Rect,
    pub original_layers: Vec<Layer>,
}
```

Engine 新增字段：

```rust
pub selection_transform: Option<SelectionTransformState>,
```

新增方法：

```rust
pub fn on_selection_transform_start(&mut self, pos: LogicalPoint, kind: ResizeHit);
pub fn on_selection_transform_drag(&mut self, pos: LogicalPoint);
pub fn on_selection_transform_end(&mut self, pos: LogicalPoint);
```

**拖拽中行为**：

- 调用 `editor.transform_selection(...)` 得到新 `Rect` 和新 `Vec<Layer>`。
- 直接覆写 `editor.selection` 和 `editor.layers` 以实现实时预览（不生成 undo 记录）。

**拖拽结束行为**：

- 比较最终状态和 `original_selection` / `original_layers`；若有变化，压入 undo 栈：
  ```rust
  LayerOp::UpdateSelectionAndLayers {
      old_selection: Some(state.original_selection),
      new_selection: editor.selection,
      old_layers: state.original_layers,
      new_layers: editor.layers.clone(),
  }
  ```
- 清空 `selection_transform`。

### 5.3 几何变换（Editor）

```rust
impl EditorState {
    /// 根据原始选区、拖拽类别和偏移计算新选区及变换后的图层。
    pub fn transform_selection(
        original_selection: Rect,
        layers: &[Layer],
        kind: &ResizeHit,
        delta: LogicalPoint,
    ) -> (Rect, Vec<Layer>) {
        // 1. 计算 new_rect
        // 2. 计算平移向量：tx = new_rect.x - original_selection.x, ty = new_rect.y - original_selection.y
        // 3. 计算缩放比例：sx = new_rect.w / original_selection.w, sy = new_rect.h / original_selection.h
        // 4. 对每一层应用：
        //    - 先相对原始选区做归一化
        //    - 再按 sx/sy 缩放
        //    - 最后加上 new_rect 的左上角
        // 5. 返回 (new_rect, new_layers)
    }
}
```

**最小尺寸限制**：计算出的新选区宽高不得低于 `MIN_SELECTION_SIZE = 8.0`，否则保持 `8.0` 并反向钳制指针偏移。

### 5.4 Undo/Redo 支持

扩展 `LayerOp`：

```rust
pub enum LayerOp {
    AddLayer { layer: Layer },
    DeleteLayer { layer: Layer },
    UpdateLayer { id: String, old: Layer, new: Layer },
    UpdateSelectionAndLayers {
        old_selection: Option<Rect>,
        new_selection: Option<Rect>,
        old_layers: Vec<Layer>,
        new_layers: Vec<Layer>,
    },
}
```

在 `EditorState::undo/redo` 中处理该操作：直接恢复 `selection` 和 `layers` 的完整快照。

### 5.5 Overlay 事件路由（app.rs）

在 `EngineState::Editing` 分支中，修改现有的 `on_edit_mouse_down` 触发逻辑：

1. 若 `engine.selection_transform` 已存在（说明正在 transform），所有鼠标事件都路由给 `on_selection_transform_drag/end`。
2. 否则，先检查 `pointer.press_origin()` 是否命中 `selection` 的 resize handle：
   - **命中**：调用 `engine.on_selection_transform_start(pos, kind)`，**不**调用 `on_edit_mouse_down`。
   - **未命中**：走原有的 `on_edit_mouse_down` 逻辑进入标注绘制。

光标样式反馈：

- 在 `Editing` 分支中，根据 `pointer.latest_pos()` 对 `selection` 做 hit test，使用 `ui.output().cursor_icon = ...` 设置对应光标：
  - `Move` -> `CursorIcon::Move`
  - `N/S` -> `CursorIcon::ResizeVertical`
  - `E/W` -> `CursorIcon::ResizeHorizontal`
  - `NW/SE` -> `CursorIcon::ResizeNwSe`
  - `NE/SW` -> `CursorIcon::ResizeNeSw`

### 5.6 Toolbar 与交互锁定

现有逻辑：`update_interactivity()` 已根据 `selection` 计算哪些显示器窗口接收鼠标事件。当用户拖动选区跨屏移动时，`selection` 会变化，因此 `update_interactivity()` 在每次 frame 中重新计算即可自然支持跨屏拖拽，无需额外修改。

Toolbar 仍只显示在包含选区中心点的那个显示器上。

## 6. API 与类型变更汇总

- `core/types.rs`：新增 `ResizeHit`、`Edge`、`Corner` 及 `Rect::hit_test_resize_handle`。
- `core/editor.rs`：新增 `transform_selection`；扩展 `LayerOp`；更新 `undo/redo`。
- `core/engine.rs`：新增 `SelectionTransformState` 及 `on_selection_transform_start/drag/end`。
- `overlay/app.rs`：在 `Editing` 分支中增加 hit test、光标设置和事件路由。

## 7. 边界情况

- **最小尺寸**：拖拽到小于 8×8 时锁为 8×8。
- **跨屏移动**：选区完全被拖到另一显示器时，toolbar 随中心点迁移到该显示器。
- **未命中时绘制**：如果用户没有点中任何手柄，则继续原来的标注绘制逻辑，不破坏现有功能。
- **没有 selection**：若 `editor.selection` 为 `None`，跳过所有 transform 逻辑。

## 8. 测试计划

- **单元测试**（`types.rs`）：`hit_test_resize_handle` 对中心、角点、边上、外部返回正确结果。
- **单元测试**（`editor.rs`）：
  - 平移选区后图层的相对位置保持不变。
  - 放大选区后图层的坐标正确按比例缩放。
  - undo/redo `UpdateSelectionAndLayers` 能完全恢复状态。
- **单元测试**（`engine.rs`）：
  - `on_selection_transform_start -> drag -> end` 产生一条 undo 记录。
  - 无任何移动直接释放不产生 undo 记录。
- **集成/手工验证**（macOS overlay）：
  - 在已有标注图层的情况下，拖动选区，图层跟随移动。
  - 拖拽四角/四边缩放，选区和图层同步缩放。
  - 光标在边框/角点变化正确。
  - Cmd+Z 能撤销最近一次拖拽。
