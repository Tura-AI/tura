# Tura 表情素材系统（系统 assets）

目录结构：

- `assets/system/tura/<expression>/meta.json`：该组表情的元信息（包含路径、方向、emoji 映射）
- `assets/system/tura/<expression>/frames/`：每组 9 张方向图（均为正方形 PNG）
- `assets/system/tura/<expression>/grid/sheet.png`：该组对应的 3x3 九宫格图（无分割线）
- `assets/system/manifest.json`：角色级映射清单

当前九宫格规则：全部 9 个格子都放图；
`up-right` 放在**右上**，`right` 放在**右侧中间**。

  - 上排：`up-left`、`up`、`up-right`
  - 中排：`left`、`center`、`right`
  - 下排：`down-left`、`down`、`down-right`

方向集合为：

- `center`
- `up`
- `down`
- `left`
- `right`
- `up-left`
- `up-right`
- `down-left`
- `down-right`

当前可用表情组（共 4 组）：

1. `panic`
2. `crying`
3. `confused`
4. `nervous`

角色 `tura` 预期有 24 组表情位；当前只存在以上 4 组，`assets/system/manifest.json` 中用 `status: "missing-assets"` 标记了其余占位位（slot 5~24）。

`manifest.json` 是映射表：
- `id`：表情组名，对应 `tura/<id>/`
- `emojiAliases`：多个 emoji 对应同一组
- `framesPath`：素材目录根路径
- `directionOrder`：九宫格顺序与渲染方向匹配
