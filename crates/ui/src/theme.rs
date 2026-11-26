//! 设计 token。数值来自 `managi-v3` 的 Nord 变量表，见 DESIGN.md §2。
//!
//! 这里只放深色主题常量；M1 会扩展为可切换的 `Palette`。

pub const BG: u32 = 0x2E34_40;
pub const PANEL_BG: u32 = 0x3B42_52;
pub const SURFACE: u32 = 0x434C_5E;
pub const BORDER: u32 = 0x4C56_6A;
pub const FONT_1: u32 = 0xD8DE_E9;
pub const FONT_2: u32 = 0xE5E9_F0;
pub const FONT_3: u32 = 0x81A1_C1;
pub const ACCENT: u32 = 0x88C0_D0;
pub const GREEN: u32 = 0xA3BE_8C;
pub const YELLOW: u32 = 0xEBCB_8B;
pub const RED: u32 = 0xBF61_6A;

/// 界面字号（px），DESIGN.md §3。
pub const TEXT_TAB: f32 = 11.2;
pub const TEXT_BAR: f32 = 12.0;
pub const TEXT_TITLE: f32 = 20.0;

/// 布局尺寸（px），DESIGN.md §4。
pub const SIDEBAR_WIDTH: f32 = 320.0;
pub const SIDEBAR_COLLAPSED_WIDTH: f32 = 192.0;
pub const BAR_HEIGHT: f32 = 32.0;
