# managi-gpui

用 Rust + GPUI 重写的原生 SSH 客户端，复用 [managi-v3](../managi-v3) 的极简 Nord 视觉。
单进程、单二进制、无 WebView，三平台同一套代码。

当前阶段：**M4（可运行客户端与 Release 流水线）**。

## 功能

- SSH 终端：密码或私钥认证、首次连接 TOFU 主机指纹、彩色与 CJK 宽字符、滚轮回看、`Ctrl+Shift+V` 粘贴、`Ctrl+Shift+C` 复制可见区、断线重连。
- SFTP：目录浏览、上一级、刷新、新建目录、上传、下载、删除文件与空目录；传输进度显示在状态栏。
- 快捷命令：终端底部开关，内置系统信息、CPU、内存、磁盘、进程、运行时长、IP、路由、监听端口、大文件、syslog、ping 共 12 条，点击即发送到当前终端。
- 批量执行：同一命令并发发到勾选的主机，逐主机显示退出码与输出，可单机停止或全部停止。
- 三平台 GitHub Actions 构建；tag 触发 Release，附带 SHA256 校验和。

## 技术栈

| 层 | 选型 |
|---|---|
| UI | GPUI（经由 `gpui-kit`，锁定 `gpui-pre` 快照） |
| 终端仿真 | `alacritty_terminal`（只借 VT 解析与网格，不启本地 PTY） |
| SSH / SFTP | `russh` / `russh-sftp`（纯 Rust，无 OpenSSL 依赖） |
| 构建 | GitHub Actions：Windows / Linux / macOS |

## 目录

```
crates/
  core/   # 配置、节点模型、路径、错误（无 UI / 无网络）
  term/   # alacritty 包装、网格快照、按键编码
  ssh/    # russh 会话、认证、known_hosts、SFTP、远程命令
  ui/     # 主题 token、侧栏、标签栏、终端、SFTP、批量执行
  app/    # 入口、窗口、CLI、日志、依赖装配
```

依赖方向单向：`app → ui → core / term`，`ssh → core`；UI 不直接依赖 `russh`。

## 下载

Release 产物按平台命名：

- `managi-gpui-windows-x64.exe`
- `managi-gpui-linux-x64`
- `managi-gpui-macos-arm64`
- 对应 `.sha256` 校验和

Windows 未签名，首次运行可能被 SmartScreen 拦截；请核对 SHA256 后选择“仍要运行”。

## 构建

本项目不要求本地编译；推送到 GitHub 后由 Actions 构建并产出可下载产物。

若要在本地开发，需要 Rust 1.92+ 与平台依赖：

- Windows：VS2022 C++ 工作负载 + Windows SDK + CMake
- Linux：Wayland/X11、Vulkan、fontconfig、xkbcommon 开发包
- macOS：Xcode（含 Metal 工具链）

```bash
cargo build --release --bins --locked
cargo test -p managi-core -p managi-term -p managi-ssh --locked
cargo run --locked --release -p managi-app --bin managi-gpui-selftest
```

## 已知限制

- 中文输入法尚未接入 GPUI 的文本输入协议，终端内暂不能直接输入中文。
- 鼠标拖选复制未实现；`Ctrl+Shift+C` 复制当前可见区而不是选中区。
- SFTP 上传使用手输本地绝对路径，尚未接原生文件选择框；不支持递归上传目录或递归删除非空目录。
- 批量执行只使用本次运行期间缓存的密码；未手动连接过的主机会提示先连接一次。
- 密码仅保存在进程内存，不落盘；主机指纹记录在 `%APPDATA%\managi-gpui\known_hosts`（Linux/macOS 为配置目录同名文件）。

## 状态与文档

- 实施计划：[PLAN.md](PLAN.md)
- 视觉规范：[DESIGN.md](DESIGN.md)

## License

MIT
