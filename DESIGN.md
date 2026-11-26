# managi-gpui 视觉规范

> 从 `managi-v3/frontend` 提取的设计语言，供 GPUI 实现逐项对照。
> 原则：**只搬设计，不搬代码**。Vue 组件与 CSS 不进入 Rust 项目，但色值、尺寸、层级、交互反馈必须一致。

## 1. 设计原则

1. **零圆角**：所有容器、按钮、输入框、菜单 `border-radius: 0`，GPUI 里不写 `.rounded_*`。
2. **细线分区**：只用 1px `#4C566A` 描边和底色差异划分层级，不用阴影（右键菜单除外，可用一层淡阴影）。
3. **小字号高密度**：主界面字号 11.2px，辅助信息 12px，终端 14px；行高紧凑但不挤。
4. **弱色界面、强色状态**：界面主体是灰蓝（Nord），只有强调色 `#88C0D0` 和状态色（绿/黄/红）承担注意力。
5. **状态只用文字 + 圆点**：连接中/已连接/失败不做遮罩层、不弹窗，放在底栏右侧。
6. **隐藏滚动条**：可滚动但不显示滚动条（终端、侧栏、文件列表一致）；GPUI 自绘 2px 细条，仅在滚动/悬停时淡入。

## 2. 颜色

### 2.1 界面（Nord Dark，默认）

| Token | 值 | 用途 |
|---|---|---|
| `accent` | `#88C0D0` | 选中文字、强调下划线、链接、图标 hover |
| `bg` | `#2E3440` | 应用底色、侧栏、标签栏底、终端底 |
| `panel_bg` | `#3B4252` | 底栏、菜单、弹窗底 |
| `surface` | `#434C5E` | 分组选中底、hover 加强底 |
| `input_bg` | `#2E3440` | 输入框底（与 bg 同色，仅靠描边区分） |
| `hover_bg` | `rgba(136,192,208,.12)` | 通用 hover |
| `selected_bg` | `rgba(136,192,208,.18)` | 分组/节点选中 |
| `border` | `#4C566A` | 所有 1px 分隔线 |
| `border_subtle` | `#434C5E` | 更弱的内部线（可选） |
| `font_1` | `#D8DEE9` | 主文字、终端前景 |
| `font_2` | `#E5E9F0` | 标题、次级强调 |
| `font_3` | `#81A1C1` | 占位符、未激活标签、计数 |
| `green` | `#A3BE8C` | 已连接、成功 |
| `red` | `#BF616A` | 失败、删除、关闭 hover |
| `orange` | `#D08770` | 警告（少用） |
| `yellow` | `#EBCB8B` | 连接中/重连中 |

### 2.2 界面（Nord Light）

| Token | 值 |
|---|---|
| `accent` | `#5E81AC` |
| `bg` | `#ECEFF4` |
| `panel_bg` | `#E5E9F0` |
| `surface` | `#FFFFFF` |
| `hover_bg` | `rgba(94,129,172,.12)` |
| `selected_bg` | `rgba(94,129,172,.18)` |
| `border` | `#D8DEE9` |
| `font_1` | `#2E3440` |
| `font_2` | `#3B4252` |
| `font_3` | `#4C566A` |

GitHub Dark / GitHub Light 两套按 v3 的 `base.css` 原值搬入，作为 3/4 号主题（可选启用）。

### 2.3 终端 16 色（Nord Dark）

| 名称 | 值 | 名称 | 值 |
|---|---|---|---|
| black | `#3B4252` | brightBlack | `#4C566A` |
| red | `#BF616A` | brightRed | `#BF616A` |
| green | `#A3BE8C` | brightGreen | `#A3BE8C` |
| yellow | `#EBCB8B` | brightYellow | `#EBCB8B` |
| blue | `#81A1C1` | brightBlue | `#81A1C1` |
| magenta | `#B48EAD` | brightMagenta | `#B48EAD` |
| cyan | `#88C0D0` | brightCyan | `#8FBCBB` |
| white | `#E5E9F0` | brightWhite | `#ECEFF4` |

终端背景 `#2E3440`，前景 `#D8DEE9`，光标 `#D8DEE9`（块状，反色文字），选区 `rgba(136,192,208,.30)`。

## 3. 排版

根字号按浏览器默认 16px 换算（v3 只在 `body` 设了 14px，rem 仍以 16px 计）。

| 用途 | rem | px | 字重 |
|---|---|---|---|
| 标签标题 / 分组名 / 节点名 / 底栏信息 | 0.7 | 11.2 | 400（分组“全部主机”600） |
| 底栏状态 | 0.75 | 12 | 400 |
| 小按钮 | 0.8 | 12.8 | 400 |
| 右键菜单项 | 0.85 | 13.6 | 400 |
| 表单控件 | 0.9 | 14.4 | 400 |
| 正文 | 0.875 | 14 | 400 |
| 设置页标题 | 1.25 | 20 | 600 |
| 终端 | 14（可配 8–32） | 14 | 300（ExtraLight） |

- 界面字体：系统 UI 字体（Windows `Segoe UI`，macOS `SF Pro Text`，Linux `Noto Sans`），CJK fallback 指定 `Microsoft YaHei UI` / `PingFang SC` / `Noto Sans CJK SC`。
- 终端字体：内置 `JetBrains Mono ExtraLight`（OFL），fallback 到 `Cascadia Mono, Consolas, Menlo, DejaVu Sans Mono`。
- 行高：界面 1.5；标签栏单行居中；终端格子高 = 字号 × 1.2（可配 1.0–1.4）。

## 4. 布局与尺寸

```
┌──────────────┬──────────────────────────────────────────────┐
│ 侧栏 320/192 │ 标签栏 32px                                   │
│              ├──────────────────────────────────────────────┤
│  搜索 32px   │                                              │
│              │  工作区（终端 / SFTP / 设置 / 批量）           │
│  分组/节点    │                                              │
│              │                                              │
│              ├──────────────────────────────────────────────┤
│ 底栏 32px    │ 终端底栏 32px（主机名 …… 状态点+文字）          │
└──────────────┴──────────────────────────────────────────────┘
```

| 元素 | 尺寸 |
|---|---|
| 侧栏展开 / 收起 | 320px / 192px（默认展开；记忆上次状态） |
| 顶栏、底栏、标签栏 | 32px |
| 标签 | 高 30px（顶部留 2px），左右内边距 12px，最小 96px，最大 224px |
| 标签图标 / 关闭按钮 | 16px / 16px |
| 新建标签按钮 | 28×28px |
| 搜索框高度 | 24px，字号 11.2px |
| 节点行 | 上下内边距 5.6px，左边缩进 24px，行间距 2px |
| 节点图标（hover 出现） | 13.6px |
| 状态点 | 4.8px 圆点（选中节点左侧 -12px 处显示绿点） |
| 终端内边距 | 8px（四边） |
| 窗口最小尺寸 | 960×600；默认 1280×800 |

## 5. 组件规格

### 5.1 侧栏

- **头部**：搜索输入 + “+” 按钮；搜索框无边框、底色同 bg，占位符 `font_3`；“+” 28px，hover 底色 `hover_bg`、图标变 `accent`。
- **分组头**：左侧 12px chevron（展开时旋转 90°），组名 11.2px，右侧计数 `font_3`；hover 底色 `hover_bg`；选中底色 `selected_bg` + 文字 `accent`；部分选中只变文字色。
- **节点行**：名称省略号截断；hover 不改变底色（v3 明确注释掉了），只在行尾出现两个图标（终端、文件管理）；选中时文字 `accent` + 左侧绿点，不填充底色。
- **底栏**：左“收起/展开”按钮，右“设置”齿轮；32px 高，上边 1px 描边。
- 右键：节点 = 编辑 / 移动到分组 / 删除；分组 = 重命名 / 删除（“全部主机”上为新建分组）。

### 5.2 标签栏

- 每个标签：类型图标（终端/文件夹/齿轮/批量）、标题、关闭 “×”。
- 激活：底色 `panel_bg`，文字 `font_1`，底部 2px `accent` 下划线（从左向右展开 180ms）。
- 未激活：底色 `bg`，文字 `font_3`；hover 底色 `tab_hover`、文字 `font_1`。
- 关闭按钮 hover 变 `red` 并放大 1.1 倍。
- 右侧可放当前标签的附加操作（v3 用 slot）。
- 右键菜单：关闭 / 关闭其他 / 关闭全部。

### 5.3 终端面板

- 纯色终端底，无边框；四周 8px 内边距。
- 底栏：左侧 `名称 (host:port)`（省略号），右侧状态：`● 正在连接…`（黄，圆点脉冲 1.2s）/ `● 已连接`（绿）/ `● 连接失败`（红）/ `未连接节点`（`font_3`）。
- 无 loading 遮罩；连接过程直接由底栏状态表达。
- 空状态：终端区居中提示“鼠标浮在左侧节点上，点击终端图标来开启一个连接吧”，绿色提示文字。

### 5.4 SFTP（第二阶段）

- 顶部 32px 工具条：面包屑路径按钮组（每个路径段是一个小按钮），右侧刷新/新建目录/上传/下载/删除 5 个 16px 图标。
- 表头：文件名（左，弹性）、大小（右，固定 8rem）、修改时间（右，固定 14rem）。
- 行高 40px，目录名 `accent`，普通文件 `font_1`；双击目录进入，双击文件下载。
- 底栏：左侧“当前目录共 N 项，当前选中…”，右侧 `节点名 + 状态点`。
- 上传/下载进行中：按钮图标替换为圆环进度（32px 内 16px 环）。

### 5.5 设置页

- 左侧 160px 导航（外观 / 终端 / 安全 / 数据 / 关于），右内容区。
- 主题为四张卡片：色块预览 + 名称，选中描边 `accent`。
- 表单项：标签在左，控件在右，行高 40px，1px 下分隔线。
- 所有控件直角、底色 `input_bg`、描边 `border`，hover 描边 `accent`。

### 5.6 弹窗与菜单

- 弹窗：`panel_bg` 底、1px `border`、无圆角、宽 420px，标题 20px 居中，底部按钮行左取消右保存（保存按钮 `accent` 底、`bg` 文字）。
- 右键菜单：`panel_bg` 底、1px `border`、13.6px 项、上下内边距 8px 12px、hover `hover_bg`；危险项 hover 文字 `red`。
- 遮罩：`rgba(0,0,0,.35)`，点击关闭。

### 5.7 按钮与输入

- 默认按钮：透明底、1px `border`、文字 `font_1`、圆角 0；hover 描边与文字变 `accent`。
- 主按钮：`accent` 底、`bg` 文字、无描边；hover 提亮 10%。
- 输入框：`input_bg` 底、1px `border`、左对齐、内边距 4px 8px；聚焦时描边 `accent`（不做外发光）。
- 禁用：透明度 0.5，光标 `not-allowed`。

## 6. 动效

| 场景 | 时长/曲线 | 说明 |
|---|---|---|
| 图标 hover 上浮 | 180ms ease，位移 -1px | 侧栏加号、标签图标、节点动作图标 |
| 标签激活下划线 | 180ms ease-out，scaleX 0→1 | 原点在左 |
| 切标签 | 160ms，opacity + scale .996→1 | 不做左右滑动 |
| 右键菜单 | 180ms，opacity + scale .92→1 | 原点左上 |
| 连接中圆点 | 1.2s 循环，透明度 1→.35、缩放 1→.75 | 唯一允许常驻的动画 |
| 侧栏折叠 | 200ms ease-in-out，宽度 320↔192 | 内容不重排跳动 |

`prefers-reduced-motion` 在桌面端对应系统“减少动画”设置：除连接中圆点外，全部降为 10ms。

## 7. 图标与资源

全部沿用 v3 的 Material Design filled 路径（Apache-2.0），以 SVG 资源打包，经 `AssetSource` 加载：

| 图标 | 用途 | 尺寸 |
|---|---|---|
| `terminal` | 标签/节点动作 | 16 / 13.6 |
| `folder` | SFTP 标签、目录行 | 16 / 24 |
| `settings` | 侧栏底、设置标签 | 16 |
| `batch` | 批量执行标签 | 16 |
| `plus` | 新建节点/标签/目录 | 16 |
| `close` | 标签关闭 | 文本 `×` |
| `chevron-right` | 分组折叠 | 12（展开旋转 90°） |
| `chevron-left` | 侧栏收起 | 16 |
| `refresh` `upload` `download` `trash` `file` | SFTP | 16 / 24 |
| `edit` `save` | 节点编辑 | 16 |

字体：`assets/fonts/JetBrainsMono-ExtraLight.ttf`（OFL-1.1）。OldTimeyCode / Honchoko Mono 是否可再分发未确认，**首版不打包**，只作为“本地字体”选项读取系统已安装版本。

## 8. CSS → GPUI 映射

| v3 (CSS) | GPUI |
|---|---|
| `display:flex; flex-direction:column` | `div().flex().flex_col()` |
| `width: 20rem` | `.w(px(320.))` |
| `height: 2rem` | `.h(px(32.))` |
| `border-bottom: 1px solid var(--border)` | `.border_b_1().border_color(cx.theme().border)` |
| `background-color: var(--hover-bg)` | `.hover(\|s\| s.bg(...))` |
| `font-size: .7rem` | `.text_size(px(11.2))` |
| `color: currentcolor` 的图标 | `svg().path(...).text_color(...)`（`currentColor` 由 gpui svg tint 实现） |
| `overflow-y:auto; scrollbar-width:none` | `.overflow_y_scroll()` + 自绘 2px 滚动条（`ScrollHandle`） |
| `transition: transform .18s` | `.with_animation("lift", Animation::new(180ms), \|el, t\| ...)` 或 hover 直接换 translate |
| `@keyframes status-pulse` | `Animation::new(1.2s).repeat()` + `percentage` |
| `position:fixed` 菜单 | `gpui::anchored` / `deferred` 浮层，`Root` 托管 |
| `input` | gpui-component `InputState` + 自绘直角外壳（见 PLAN R4） |

## 9. 桌面端差异（有意偏离 Web 版）

1. 原生标题栏（Windows 深色标题栏 / macOS 交通灯），不做自绘拖动区；标题显示 `managi-gpui`。
2. 字体渲染走系统栅格化，不使用 `-webkit-font-smoothing`；小字号下限 11px，避免 Windows 上过细。
3. 右键菜单为原生窗口内浮层，边界自动避让屏幕。
4. Web 的 `localStorage` 改为 `config.toml`；密码改存系统钥匙串（见 PLAN §4.5）。
5. 快捷键桌面化：`Ctrl+N` 新建终端、`Ctrl+W` 关闭标签、`Ctrl+Tab` 切换、`Ctrl+,` 设置、`Ctrl+Shift+V` 粘贴、`Ctrl+K` 清屏。
6. 窗口缩放时终端立即重排（30ms 去抖），不做浏览器式的整页重排。

## 10. 视觉验收清单

- [ ] 侧栏 320px / 收起 192px，切换无跳动
- [ ] 顶栏、标签栏、底栏均为 32px，边界 1px `#4C566A`
- [ ] 所有控件圆角为 0，无投影（菜单除外）
- [ ] 标签激活下划线 2px `#88C0D0`，从左展开
- [ ] 节点 hover 只出现两个动作图标，不变底色
- [ ] 连接中状态为黄色脉冲圆点，已连接为绿色静止圆点
- [ ] 终端底色 `#2E3440`，前后景与 16 色一致
- [ ] 滚动条默认不可见，滚动时 2px 细条淡入
- [ ] 中英文切换后布局不溢出（标签/按钮宽度自适应）
- [ ] 100%/125%/150% 缩放下无半像素模糊线
