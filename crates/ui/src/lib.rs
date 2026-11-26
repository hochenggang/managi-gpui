//! GPUI 界面层：主题、侧栏、标签栏、工作区、终端、设置与弹窗。
//!
//! 依赖方向：`ui` 依赖 `core`，并通过 `ssh`/`term` 的稳定接口驱动会话；
//! 不直接接触 russh 或 alacritty 的内部类型。

mod app_view;
mod assets;
mod batch_runtime;
mod batch_view;
mod i18n;
mod menu;
mod modal;
mod settings_view;
mod sftp_runtime;
mod sftp_view;
mod sidebar;
mod state;
mod tab_bar;
mod terminal_runtime;
mod terminal_view;
mod theme;
mod widgets;
mod workspace;

pub use app_view::AppView;
pub use assets::Assets;
