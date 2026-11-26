//! GPUI 界面层：主题、侧栏、标签栏、工作区、设置与弹窗。
//!
//! 依赖方向：`ui` 依赖 `core`；`app` 装配 `ui`。`ui` 不反向依赖网络/终端。

mod app_view;
mod assets;
mod i18n;
mod menu;
mod modal;
mod settings_view;
mod sidebar;
mod state;
mod tab_bar;
mod theme;
mod widgets;
mod workspace;

pub use app_view::AppView;
pub use assets::Assets;
