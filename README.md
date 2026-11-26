# managi-gpui

用 Rust + GPUI 重写的原生 SSH 客户端，复用 [managi-v3](../managi-v3) 的极简 Nord 视觉。
单进程、单二进制、无 WebView，三平台同一套代码。

当前阶段：**M0（骨架 + 三平台 CI）**。

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
  ssh/    # russh 会话、认证、known_hosts
  ui/     # 主题 token、侧栏、标签栏、终端视图
  app/    # 入口、窗口、CLI、日志、依赖装配
```

依赖方向单向：`app → ui → core / term`，`ssh → core`；UI 不直接依赖 `russh`。

## 构建

本项目不要求本地编译；推送到 GitHub 后由 Actions 构建并产出可下载产物。

若要在本地开发，需要 Rust 1.92+ 与平台依赖：

- Windows：VS2022 C++ 工作负载 + Windows SDK + CMake
- Linux：Wayland/X11、Vulkan、fontconfig、xkbcommon 开发包
- macOS：Xcode（含 Metal 工具链）

```bash
cargo build --release
cargo test -p managi-core -p managi-term
cargo run -p managi-app --bin managi-gpui-selftest
```

## 状态与文档

- 实施计划：[PLAN.md](PLAN.md)
- 视觉规范：[DESIGN.md](DESIGN.md)

## License

MIT
