# managi-gpui 实施计划

> 版本 v1・2026-11-26
> 目标目录：
>
> `C:\Users\Administrator\Documents\codes\managi\managi-gpui`
> 构建方式：
>
> **只在 GitHub Actions 编译**
>
> 。本机 rustc 1.83，低于 GPUI 要求的 1.92，且按你的要求不做本地编译。



***

## 0. 结论



1. **方案可行**。GPUI 有 crates.io 快照（`gpui-pre 0.3.14`）+ 门面 crate `gpui-kit 0.11.0`（2026-11-23 发布），Windows /macOS/ Linux 同一套代码；三平台都能在 GitHub 托管 runner 上编译。

2. **终端不自己写 VT**：用 `alacritty_terminal 0.26`，只借它的解析器和网格，不启本地 PTY；把 SSH 通道的字节流喂进去即可。

3. **SSH 用&#x20;**`russh 0.65.4`：纯 Rust、无 OpenSSL/C 依赖，CI 不需要额外库；SFTP 用 `russh-sftp 4.0`。

4. **视觉搬设计不搬代码**：Nord 色板、320/192px 侧栏、32px 上下栏、0 圆角、1px 描边、11.2px 小字、2px 强调下划线、隐藏滚动条。逐项规格见 [DESIGN.md](DESIGN.md)。

5. 分 5 个里程碑，**每个里程碑都产出可下载的 Windows 产物**。M1 结束你做第一次视觉验收，M2 结束做第一次真机连通验收。

6. 需要你先做 4 件事：建空仓库、开 Actions 写权限、生成受限 PAT 存到本地文件、把仓库全名告诉我。见 §8、§9。



***

## 1. 调研结论

### 1.1 GPUI 版本路线



| 路线                      | 版本                  | 优点                                                   | 问题                    | 取舍     |
| ----------------------- | ------------------- | ---------------------------------------------------- | --------------------- | ------ |
| Zed 仓库 git 依赖           | 每日变动                | 最新 API                                               | 不可复现、CI 拉全仓慢、破坏性更新    | 不用     |
| **gpui-kit → gpui-pre** | **0.11.0 → 0.3.14** | crates.io 可复现；一个依赖含 GPUI + Base + Component + 图标；三平台 | 仍是 pre-1.0，升级需跟随      | **采用** |
| `gpui`（crates.io 独立包）   | 0.2.2（2025-10）      | 有现成 GPUI 终端示例可参考                                     | 比 0.3.14 老约一年，缺少新 API | 仅作参考   |

已核对 crate 源码，确认本项目要用的 API 形态：



* `TextSystem::shape_line(text, font_size, &[TextRun], force_width) -> ShapedLine`

* `TextRun { len, font, color, background_color, underline, strikethrough }`（终端按段着色 / 画底色就靠它）

* `Font { family, fallbacks: Option<FontFallbacks>, weight, style }`（中文字体 fallback 可控）

* `TextSystem::add_fonts(Vec<Cow<'static, [u8]>>)`（内嵌 JetBrains Mono）

* `AssetSource`（SVG 图标资源）

### 1.2 终端仿真



| 方案                           | 结论                                                                                                                                                                                                                               |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **alacritty\_terminal 0.26** | 采用。`Term::new(Config, &SizeInfo, VoidListener)`、`vte::ansi::Processor::advance(&mut term, bytes)`、`renderable_content()`（含 `display_iter / cursor / selection / colors / mode`）、`Term::resize`、`selection_to_string`；不依赖 PTY，正合适 |
| vt100                        | 功能太少：无完整 scrollback / 选择 / 宽字符处理                                                                                                                                                                                                 |
| wezterm-term                 | 依赖面大，编译成本高                                                                                                                                                                                                                       |

渲染层自己写：参照 oxide 的 “按行分段 shape\_line” 思路，但按 0.3.14 API 重写，不引入对方代码。

### 1.3 SSH / SFTP



| 项                | 结论                                                                                                                                                                                                                                                          |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **russh 0.65.4** | `client::connect(config, addrs, handler)`；`Handler::check_server_key(&PublicKeyOrCertificate)`；`authenticate_password` / `authenticate_publickey`；`channel_open_session` → `request_pty` → `request_shell` → `data`；`window_change`；自带 keepalive/inactivity |
| 主机密钥             | `known_hosts::check_known_hosts_path(host, port, key, path)` 校验；`learn_known_hosts_path` 做 TOFU 落盘；支持指向 `~/.ssh/known_hosts` 严格模式                                                                                                                           |
| 备选 ssh2          | libssh2 C 依赖，三平台 CI 与交叉编译都麻烦，不用                                                                                                                                                                                                                             |
| SFTP             | `russh-sftp 4.0`：`SftpSession::new(channel.into_stream())`，读写 / 列目录 / 删除 / 改名齐全                                                                                                                                                                             |

### 1.4 CI 可行性



| Runner         | 需要                                                                                                      | 备注                                                                             |
| -------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `windows-2022` | VS2022 C++ 工作负载 + Windows SDK + CMake                                                                   | 镜像自带；MSVC 工具链，不能 GNU                                                           |
| `ubuntu-24.04` | gcc/g++/clang、pkg-config、fontconfig、wayland、xkbcommon-x11、x11-xcb、vulkan、webkit2gtk-4.1、ssl、zstd、asound | 一条 apt 命令；GPUI 用 Wayland/X11 + Vulkan，不用 WebKit 渲染，但保留依赖包防链接失败                 |
| `macos-15`     | Xcode（含 Metal 工具链）                                                                                      | 若镜像 Xcode 26 缺 MetalToolchain，需 `xcodebuild -downloadComponent MetalToolchain` |



* 冷构建约 15–25 分钟（GPUI 体量大）；缓存命中约 4–8 分钟。

* 公开仓库的 standard runner 不计费；GitHub 默认保留产物 90 天，本项目 CI artifact 显式设 30 天，Release 附件永久。

### 1.5 依赖锁定（草案）



```
[workspace.dependencies]
gpui-kit = "=0.11.0"                 # 内部已 pin gpui-pre =0.3.14
alacritty_terminal = "=0.26.0"
russh = "=0.65.4"
russh-sftp = "=4.0.0"
tokio = { version = "1", features = ["rt-multi-thread", "net", "time", "sync", "macros", "io-util"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "1"
thiserror = "2"
tracing = "0.1"
directories = "6"
keyring = "4"
```

`rust-version = "1.92"`；edition 2024。所有版本用 `=` 精确锁定，避免上游 API 漂移把 CI 弄红。

参考来源：



* [GPUI Kit 文档（安装 / Getting Started）](https://gpui-kit.com/zh-CN/docs/installation/)

* [gpui-component（GitHub）](https://github.com/longbridge/gpui-component)

* [GPUI 仍是 pre-1.0 的现状说明](https://www.salamunteknologi.com/en/blog/rust-gpui-desktop-apps)

* [russh Channel API](https://docs.rs/russh/0.65.4/russh/struct.Channel.html)



***

## 2. 设计复用

完整规格见 [DESIGN.md](DESIGN.md)，这里只列落地要点：



| 项       | v3 值                                | GPUI 实现                                     |
| ------- | ----------------------------------- | ------------------------------------------- |
| 侧栏      | 320px / 收起 192px                    | `.w(px(320.))`，状态存配置                        |
| 上下栏     | 32px                                | `.h(px(32.))`                               |
| 字号      | 11.2 / 12 / 12.8 / 13.6 / 14.4 / 14 | `.text_size(px(..))`                        |
| 分隔线     | 1px `#4C566A`                       | `.border_b_1().border_color()`              |
| 圆角      | 0                                   | 不写 `.rounded_*`                             |
| 图标      | Material filled SVG                 | `svg().path("icons/x.svg")` + `AssetSource` |
| 状态灯     | 4.8px 圆点 + 文字                       | 自绘 `div().size(px(4.8)).rounded_full()`     |
| 终端 16 色 | Nord                                | `TerminalTheme` 常量表                         |

桌面端有意差异（原生标题栏、系统字体栈、钥匙串存密码、桌面快捷键）也在 DESIGN §9 里写明。



***

## 3. 范围

### 3.1 MVP（v0.1.0，M0–M2）



* 主机与分组管理：增删改、搜索、分组折叠、右键菜单、拖分组（先做 “移动到分组” 菜单项）

* 认证：密码 / 私钥（含 passphrase）；主机密钥 TOFU + 严格模式

* 多标签终端：新建、切换、关闭、关闭其他 / 全部；窗口自适应；断线自动重连

* 终端能力：滚动回看、选择复制、粘贴、Ctrl 组合键、方向键 / F 键、宽字符、中文输出

* 外观：Nord Dark/Light + GitHub Dark/Light；中 / 英切换

* 设置：字号、字体、主题、语言；配置导入 / 导出（兼容 v3 JSON）

* 安全：密码存系统钥匙串；日志与崩溃记录

### 3.2 第二阶段（v0.2.x，M3–M4）



* SFTP：列目录、进入 / 返回、上传、下载、删除、新建目录、进度环

* 快捷命令侧栏 + 批量执行（多主机并发 exec、stdout/stderr 分流、退出码、可停止）

* 终端增强：鼠标上报（vim/htop）、搜索、超链接

### 3.3 明确不做

本地 shell、SSH 端口转发、多级跳板、插件系统、云同步、Web/WASM 版、移动端。

理由：每项都会显著扩大状态面与测试面；先把 “打开就能连、看着像 managi” 做扎实。



***

## 4. 架构

### 4.1 Workspace 与边界



```
managi-gpui/
├── Cargo.toml                 # workspace + 统一依赖 + release profile
├── crates/
│   ├── core/                  # managi-core：模型、配置、路径、错误、导入导出（无 gpui/无 tokio）
│   ├── term/                  # managi-term：alacritty 包装、快照、按键编码、选择（无 gpui）
│   ├── ssh/                   # managi-ssh：russh 会话、认证、known_hosts、exec、sftp
│   ├── ui/                    # managi-ui：主题 token、侧栏、标签栏、终端视图、弹窗（依赖 gpui-kit）
│   └── app/                   # managi-app：入口、窗口、CLI、日志、依赖装配
├── assets/                    # icons/、fonts/、themes/
├── .github/workflows/         # ci.yml、release.yml
├── DESIGN.md
├── PLAN.md
└── README.md / LICENSE
```

依赖方向（单向，禁止回指）：



```
app ──> ui ──> core
 │       └──> term
 └─────> ssh ──> core
```



* `ui` **不许** import `russh`；它只依赖 `core` 里的 `SessionService` trait。

* `ssh` 实现该 trait；`app` 负责把实现注入 UI。

* `term` 不依赖 gpui，保证可单元测试（ANSI 解析、按键编码都能在 CI 跑）。

### 4.2 线程与数据流



```
gpui 主线程（UI 状态、绘制、输入）
   │  SessionCommand（有界 channel）
   ▼
tokio 运行时（后台线程，2 worker）
   │  russh：连接/认证/PTY/读写
   ▼  ChannelMsg::Data（原始字节）
解析线程（每会话 1 个 std::thread，栈 256KB）
   │  vte::ansi::Processor::advance → alacritty Term（FairMutex）
   ▼  TerminalSnapshot（只拷贝可见区 + 光标 + 模式）
gpui 主线程：合并 4–8ms 的变更通知 → 重绘
```

反向：键盘事件 → `encode_key(KeyInput, TermMode)` → `SessionCommand::Write` → russh `data()`。

UI 线程永不等待网络、永不长时间持有终端锁；解析线程不做任何 UI 调用。

### 4.3 关键接口（草案）



```
// core::session
pub trait SessionService: Send + Sync + 'static {
    fn connect(&self, req: ConnectRequest) -> Task<SessionId>;
    fn write(&self, id: SessionId, bytes: Vec<u8>);
    fn resize(&self, id: SessionId, cols: u16, rows: u16);
    fn disconnect(&self, id: SessionId, reason: DisconnectReason);
}

pub enum SessionEvent {
    Connecting,
    Authenticating,
    Connected { banner: Option<String> },
    Data(Vec<u8>),
    Closed(CloseReason),
}

// term
pub fn encode_key(key: &KeyInput, mode: TermMode) -> Option<Vec<u8>>;
pub struct TerminalModel { /* Term + Processor + 尺寸 */ }
impl TerminalModel {
    pub fn advance(&mut self, bytes: &[u8]);
    pub fn snapshot(&self) -> TerminalSnapshot;
    pub fn resize(&mut self, cols: u16, rows: u16);
    pub fn selection_text(&self) -> Option<String>;
}

// core::store
pub trait NodeStore { /* 增删改查、分组、移动 */ }
pub trait SecretStore { fn get(&self, id: NodeId) -> Result<Option<String>>; fn set(...); fn delete(...); }
```

### 4.4 终端渲染与输入



* 格子宽：`text_system.advance(font_id, font_size, 'm')`；格子高：字号 × 1.2（可配）。

* 每行把相邻同色同底的 cell 合并成一个 `TextRun`；纯空格段只画背景 quad，不发字形。

* 光标、选区、下划线、删除线各自用 quad / 装饰绘制；宽字符占 2 格，第二格跳过。

* 滚动条：2px，自绘，滚动或悬停时淡入，停止 800ms 后淡出。

* 输入映射：


  * 可打印字符（含 IME 提交文本）

  * Ctrl+A…Z → 0x01–0x1A；Ctrl+Space → 0x00；Ctrl+\[ → ESC

  * 方向键：普通 `CSI A/B/C/D`，应用光标模式（DECCKM）切 `SS3 A/B/C/D`

  * Home/End/PgUp/PgDn/Del/Ins/F1–F12 标准 xterm 序列

  * Alt 作为 ESC 前缀

  * 粘贴：bracketed paste 开启时包 `ESC[200~ … ESC[201~`；换行统一 `\r`；>64KB 截断并提示

* 尺寸变化：去抖 30ms → `Term::resize` + `window_change(cols, rows, 0, 0)`。

* IME：M2 先覆盖 ASCII 与粘贴；中文输入在 M3 用隐藏 `InputState` 做组合代理，稳定后替换为自实现 `EntityInputHandler`（风险 R4）。

### 4.5 配置、密钥与主机信任



| 内容        | 位置                                                                                                                            |
| --------- | ----------------------------------------------------------------------------------------------------------------------------- |
| 配置        | Windows `%APPDATA%\managi-gpui\config.toml`；macOS `~/Library/Application Support/managi-gpui/`；Linux `~/.config/managi-gpui/` |
| 主机密钥      | 同目录 `known_hosts`（OpenSSH 格式，可配置指向 `~/.ssh/known_hosts`）                                                                      |
| 日志        | 同目录 `logs/managi-gpui.log`（5MB × 3 滚动）                                                                                        |
| 密码 / 私钥口令 | `keyring` v4：Windows 凭据管理器 /macOS Keychain / Linux Secret Service；service `managi-gpui`，account = 节点 UUID；取不到则仅本次会话，不落盘       |
| 私钥文件      | 只存路径，不复制内容                                                                                                                    |

`config.toml` 只存非敏感字段：`id/name/host/port/username/auth_kind/group/created_at` + `settings`。

v3 JSON 导入时：`auth_value` 写入钥匙串，`auth_type` 映射为 `auth_kind`，其余字段直搬。

### 4.6 稳健性约定



* 超时：TCP 10s、握手 15s、认证 20s；DNS 由 tokio 解析，10s 上限。

* keepalive 30s、连续 3 次无响应断开；异常断线按 1/2/4/8/16s 退避重连，最多 5 次；用户主动关闭不重连。

* 背压：每会话输出队列上限 8MB，满则暂停读通道；输入队列上限 1MB。

* 错误：`thiserror` 分域（Config/Ssh/Term/Ui）；面向用户的文案是短句中文，技术细节只进日志；UI 层禁止 `unwrap/expect`（启动期除外）。

* 日志：`tracing` + 自写滚动 writer；连接、认证、断开、重连、SFTP 操作都记一行摘要。

* 崩溃：release 保留 `panic = "unwind"`，装 panic hook 写崩溃日志（不用 `abort`，否则没有现场）。

* 自检：`managi-gpui --selftest` 不建窗口，跑配置读写、known\_hosts 校验、ANSI 快照、按键编码，退出码 0/1，CI 三平台都跑。



***

## 5. 里程碑



| 阶段                 | 内容                                                               | 验收（谁验）                                                              | 预计 CI 轮次 | 产物          |
| ------------------ | ---------------------------------------------------------------- | ------------------------------------------------------------------- | -------- | ----------- |
| **M0 骨架 + CI**     | workspace、空窗口、`--selftest`、ci.yml、LICENSE/README/.gitignore      | 三平台 job 全绿；Windows artifact 能启动出窗口；`--selftest` 三平台通过（你跑 exe 确认）    | 1–3      | Windows zip |
| **M1 视觉外壳**        | 主题 token、侧栏（搜索 / 分组 / 节点 / 折叠 / 右键）、标签栏、底栏、空状态、设置页、节点增删改、导入导出、中英 | 与 v3 预览并排：色值一致、关键尺寸误差 ≤2px、0 圆角、hover / 选中状态一致（你截图验收）               | 5–8      | Windows zip |
| **M2 终端（MVP）**     | russh 密码 / 私钥、PTY、解析线程、渲染、输入、滚动、复制粘贴、resize、重连、TOFU              | 真机连 2–3 台：`vim`/`htop`/`tmux` 正常；`ls --color`、中文、宽字符、断网重连、窗口缩放（你实测） | 8–12     | Windows zip |
| **M3 SFTP + 快捷命令** | 浏览 / 上传 / 下载 / 删除 / 新建目录、进度、断线提示；快捷命令侧栏                          | 传 100MB 文件、目录进出、权限错误提示（你实测）                                         | 4–6      | Windows zip |
| **M4 批量执行 + 发布**   | 多主机并发 exec、结果卡片、停止；tag 发布三平台                                     | 3 台并发、退出码正确、可中止；Release 产物 + SHA256 可下载                             | 2–4      | Release 全平台 |

每个阶段合并到 `main` 都会顺带产出 artifact，方便你随时试。



***

## 6. CI/CD 设计

### 6.1 `ci.yml`



* 触发：`push`（main / 任意分支）、`pull_request`、`workflow_dispatch`

* `concurrency`: 同 ref 取消旧任务

* matrix：`windows-2022` / `ubuntu-24.04` / `macos-15`

* 步骤：

1. `actions/checkout@v4`

2. Linux：`apt-get install` GPUI 依赖（见 §1.4）

3. `dtolnay/rust-toolchain@stable`（`rustfmt`、`clippy` 组件）

4. `Swatinem/rust-cache@v2`（`cache-on-failure: true`）

5. macOS：按需 `xcodebuild -downloadComponent MetalToolchain`

6. `cargo build --release`

7. `cargo test --workspace`（纯逻辑 crate 为主）

8. `./target/release/managi-gpui --selftest`

9. 上传产物（zip/tar.gz，保留 30 天）

10. 上传 `Cargo.lock` 作为 artifact（首次生成后由我下载并提交，之后用 `--locked`）

质量门渐进：M0 只要求 build；M1 起加 clippy（先不阻断）；M2 起 `-D warnings` + test 阻断。

### 6.2 `release.yml`



* 触发：`push tag v*`

* `permissions: contents: write`

* 三平台 release 构建 → 打包：



| 平台          | 产物                                                        |
| ----------- | --------------------------------------------------------- |
| Windows x64 | `managi-gpui-windows-x64.zip`（exe + LICENSE + README）     |
| Linux x64   | `managi-gpui-linux-x64.tar.gz`（binary + LICENSE + README） |
| macOS arm64 | `managi-gpui-macos-arm64.zip`（裸 binary + 打开说明）            |
| 公共          | `SHA256SUMS.txt`                                          |



* 用 `softprops/action-gh-release` 或 `gh release create` 建 Release，附自动生成的变更说明。

### 6.3 体积与性能预算



* Windows exe ≤ 25MB（zip ≤ 12MB）；Linux ≤ 30MB；macOS ≤ 25MB。

* 超预算时按序砍：关闭 gpui-component 的 `syntect`/`tree-sitter`/`speech` feature → `opt-level="s"` → 去掉未用组件。

* release profile：`lto="fat"`、`codegen-units=1`、`strip="symbols"`、`panic="unwind"`（保留崩溃日志）。



***

## 7. 分工



| 事项                | 谁           | 具体动作                                                                                         | 完成信号         |
| ----------------- | ----------- | -------------------------------------------------------------------------------------------- | ------------ |
| 建公开空仓库            | **你**       | [github.com/new](https://github.com/new)，名字 `managi-gpui`，Public，不勾 README/gitignore/license | 页面出现空仓库      |
| 开 Actions 写权限     | **你**       | Settings → Actions → General → Workflow permissions → Read and write                         | 保存成功         |
| 生成受限 PAT          | **你**       | 见 §8 第 3 步，只授权该仓库                                                                            | token 存到本地文件 |
| 告诉仓库全名            | **你**       | 回复 `owner/managi-gpui`                                                                       | 我拿到          |
| 写代码 / CI / 文档     | **我**       | 全部实现、推送、读日志修编译、下载产物核体积                                                                       | 每阶段绿灯        |
| 自动调试              | **我**       | 读 Actions 日志 → 定位编译错误 → 改 → 推送，循环                                                            | 构建通过         |
| 视觉验收              | **你**       | 下载 M1 产物，截图与 v3 对比，指出偏差                                                                      | 你回复截图 / 问题   |
| 真机连通验收            | **你**       | 提供测试机或自己连，按 §5 M2 清单试                                                                        | 你回复通过 / 报错   |
| 编译 / 测试 / 打包 / 发版 | **Actions** | 三平台矩阵、缓存、artifact、Release                                                                    | 绿灯 + 产物      |

你的总投入预估：首次准备 10 分钟；之后每个里程碑 5–15 分钟试跑与截图。



***

## 8. 你需要在 GitHub 做的（按顺序）

**第 1 步：建仓库**



1. 打开 [https://github.com/new](https://github.com/new)

2. Repository name 填 `managi-gpui`

3. 选 **Public**

4. **不要**勾选 “Add a README file”“Add .gitignore”“Choose a license”（这些我随后写，避免首推冲突）

5. Create repository

**第 2 步：开 Workflow 写权限**

仓库页 → Settings → Actions → General → 拉到 “Workflow permissions” → 选 **Read and write permissions** → Save。

（Release 上传产物需要它；不开会 403。）

**第 3 步：生成细粒度 PAT**

头像 → Settings → Developer settings → Personal access tokens → **Fine-grained tokens** → Generate new token：



* Token name：`managi-gpui-agent`

* Expiration：90 days（到期我会提前提醒你换）

* Repository access：**Only select repositories** → 只选 `managi-gpui`

* Permissions：


  * **Contents**：Read and write（推代码）

  * **Workflows**：Read and write（改 `.github/workflows/*` 必须）

  * **Actions**：Read and write（读构建日志、重跑失败 job）

  * **Metadata**：Read-only（自动勾选）

* Generate token → 复制

**第 4 步：把 token 存成本地文件（不要贴在聊天里）**

在 PowerShell 里执行（把 `github_pat_xxx` 换成你的 token）：



```
New-Item -ItemType Directory -Force "$env:USERPROFILE\.secrets" | Out-Null
Set-Content -Path "$env:USERPROFILE\.secrets\managi-gpui-pat.txt" -Value 'github_pat_xxx' -NoNewline -Encoding utf8NoBOM
# 仅当前用户可读（可选）
icacls "$env:USERPROFILE\.secrets\managi-gpui-pat.txt" /inheritance:r /grant:r "$($env:USERNAME):(R)" | Out-Null
```

文件要求：单行、无引号、无空格、无换行、UTF-8 无 BOM。

**第 5 步：告诉我**

回复两件事：`已放好` + 仓库全名（例如 `hochenggang/managi-gpui`）。不需要发 token 内容。

> 备选：你更愿意用浏览器授权的话，装 
>
> `gh`
>
> （
>
> `winget install GitHub.cli`
>
> ）后执行 
>
> `gh auth login`
>
> ，选 HTTPS 并在浏览器授权；之后我用 
>
> `gh`
>
>  调 API，就不需要 PAT 文件。二选一即可。



***

## 9. 凭据与权限



| 需要             | 用途                               | 最小权限                                                      | 存放                                                                       |
| -------------- | -------------------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------ |
| GitHub 细粒度 PAT | 推代码、改 workflow、读 Actions 日志、下载产物 | 仅该仓库：Contents RW / Workflows RW / Actions RW / Metadata R | `%USERPROFILE%\.secrets\managi-gpui-pat.txt`                             |
| 仓库全名           | 拼 remote 与 API 路径                | —                                                         | 聊天里给我                                                                    |
| 测试 SSH 主机（可选）  | M2/M3 功能验收                       | 一个可丢弃的账号                                                  | `%USERPROFILE%\.secrets\ssh-test.txt`，格式 `host:port user auth`，不提交、不进 CI |

**不需要**：GitHub 密码 / 2FA 验证码、你服务器的 root 密码、私钥原文、Apple 开发者证书、Windows 代码签名证书（首版不签名）。

**安全约定**：



* token 只在命令行临时变量里出现，不写 `.git/config`、不打印、不提交。

* `.gitignore` 覆盖 `.secrets/`、`*.pat`、`.env`；CI 不配置任何 secret，发版用 `GITHUB_TOKEN` 临时令牌。

* 我读取 token 文件但不会在回复里回显；发现泄露可立即在 GitHub 撤销并换新文件。



***

## 10. 风险与对策



| 编号  | 风险                                | 影响                  | 对策                                                              |
| --- | --------------------------------- | ------------------- | --------------------------------------------------------------- |
| R1  | GPUI pre-1.0，0.11 → 0.12 可能改 API  | 升级返工                | 精确锁版本；升级单独开 PR；不跟 git 主分支                                       |
| R2  | 首次 CI 编译 15–25 分钟                 | 迭代慢                 | rust-cache；一次推送批量修多个错误；先跑 M0 验证工具链                              |
| R3  | 二进制可能 30MB+                       | 偏离 “超轻量”            | 关可选 feature、LTO、strip；CI 输出体积报告，超预算即砍                           |
| R4  | 终端中文 IME / 宽字符                    | 中文输入不可用             | `Font::fallbacks` 指定 CJK；宽字符 2 格处理；M3 用隐藏 InputState 代理；加单元测试   |
| R5  | macOS 未签名                         | 首次打开被 Gatekeeper 拦  | README 写 “右键打开 / `xattr -d com.apple.quarantine`”；后续可加签名（需你的证书） |
| R6  | Linux 运行需 Vulkan 驱动               | 用户机器起不来             | README 注明 `libvulkan1` + 驱动；CI 只保证编译                            |
| R7  | CI 无交互桌面，无法真跑 GUI                 | 视觉 / 交互 bug 到你这儿才发现 | `--selftest` 覆盖逻辑；每阶段产物交你试跑截图；必要时加 gpui headless 测试             |
| R8  | 钥匙串在 Linux 可能不可用                  | 密码无法保存              | 降级为 “仅本次会话”，设置页明确提示                                             |
| R9  | alacritty\_terminal 0.26 API 细节偏差 | 返工                  | 已下载 0.26 源码核对 `Term/Processor/RenderableContent`；实现前再核一次        |
| R10 | 公开仓库代码含内部设计                       | 信息外泄                | 只放通用 SSH 客户端代码；不带 managi-v3 的后端 / 部署 / 业务代码；截图不含真实主机信息          |



***

## 11. 验收指标（目标值）



| 指标       | 目标                                           |
| -------- | -------------------------------------------- |
| 冷启动到窗口可见 | < 800ms（Windows 中端机）                         |
| 空闲 CPU   | < 1%（单会话静置）                                  |
| 内存       | < 150MB（单会话 + 10000 行回看）                     |
| 键盘输入到回显  | < 16ms（本地网络）                                 |
| 满屏输出     | 不丢帧、不卡 UI（4–8ms 合批）                          |
| 窗口缩放     | 终端重排 < 50ms                                  |
| 二进制      | Windows ≤ 25MB / Linux ≤ 30MB / macOS ≤ 25MB |
| 稳定性      | `--selftest` 三平台通过；连续 8 小时会话不崩               |



***

## 12. 节奏与下一步



* M0：收到你的仓库信息后当天完成（1–2 次 CI）。

* M1：约 2–3 个工作日（含你 1 次截图反馈）。

* M2：约 4–6 个工作日（含你 1–2 次真机反馈）。

* M3/M4：各 3–4 个工作日。

* 总计约 2–3 周，主要成本是 CI 排队与真机验收往返。

**现在等你**：§8 的 1–4 步 + 回复仓库全名。收到后我直接开 M0，先把 “三平台能编译 + Windows 能开窗” 的骨架推上去。