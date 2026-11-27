# managi-gpui

用 Rust + GPUI 重写的原生 SSH 客户端，复用 [managi-v3](../managi-v3) 的极简 Nord 视觉。
单进程、单二进制、无 WebView，三平台同一套代码。

当前阶段：**M4（可运行客户端与 Release 流水线）**。

## 功能

- SSH 终端：密码、私钥或自动认证、首次连接 TOFU 主机指纹、彩色与 CJK 宽字符、滚轮回看、鼠标拖选与双击/三击选词选行、`Ctrl+Shift+C` 复制选区（无选区时复制可见区）、`Ctrl+F` 搜索输出、`Ctrl+Click` 打开链接、右键菜单、断线重连。
- SSH 配置集成：读取 `~/.ssh/config`（支持 `Include` 与 `Host *` 默认项）并在侧栏“SSH 配置”分组只读展示；新增/编辑主机写入托管片段（默认 `~/.ssh/managi/config`，已有 `config.d` 时写入该目录），主配置只补一行 `Include`。
- 凭据：密码与私钥口令存入系统钥匙串（Windows 凭据管理器 / macOS 钥匙串 / Linux Secret Service），输入一次后可直接连接。
- SFTP：目录浏览、上一级、刷新、新建目录、上传、下载、删除文件与空目录；传输进度显示在状态栏。
- 快捷命令：终端底部开关或 `Ctrl+Shift+K` 打开，内置系统信息、CPU、内存、磁盘、进程、运行时长、IP、路由、监听端口、大文件、syslog、ping 共 12 条，点击即发送到当前终端。
- 批量执行：同一命令并发发到勾选的主机，逐主机显示退出码与输出，可单机停止或全部停止。
- 设置页支持导出/导入配置（导入为合并节点与分组，不覆盖当前设置）。
- 三平台 GitHub Actions 构建；tag 触发 Release，附带 SHA256 校验和。

## 技术栈

| 层 | 选型 |
|---|---|
| UI | GPUI（经由 `gpui-kit`，锁定 `gpui-pre` 快照） |
| 终端仿真 | `alacritty_terminal`（只借 VT 解析与网格，不启本地 PTY） |
| SSH / SFTP | `russh` / `russh-sftp`（纯 Rust，无 OpenSSL 依赖） |
| 凭据存储 | `keyring`（系统凭据管理器 / 钥匙串 / Secret Service） |
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
- 终端搜索只做当前网格与回看历史的纯文本匹配，不做正则；`Ctrl+F` 打开，`Enter`/`Shift+Enter` 跳下一/上一处。
- 链接识别只认 `http://` 与 `https://`，需要按住 `Ctrl` 点击；没有悬停下划线提示。
- 系统 `~/.ssh/config` 中带 `ProxyJump` / `ProxyCommand` 的主机暂不展示；系统原始 Host 只读，不覆盖同名 alias。
- 私钥口令的“首次提示”尚未接入：加密私钥若钥匙串里没有口令，会直接认证失败。
- SFTP 上传使用手输本地绝对路径，尚未接原生文件选择框；不支持递归上传目录或递归删除非空目录。
- 批量执行优先使用钥匙串/本次运行缓存的密码；两者都没有的主机会提示先连接一次。
- 主机指纹优先写入系统 `~/.ssh/known_hosts`；`~/.ssh` 目录不存在时回退到应用配置目录。

## 状态与文档

- 实施计划：[PLAN.md](PLAN.md)
- 视觉规范：[DESIGN.md](DESIGN.md)

## License

MIT
