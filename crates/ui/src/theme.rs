//! 设计 token 与四套色板。数值来自 `managi-v3/frontend/src/assets/base.css`。
//!
//! 不把颜色散落在组件里：所有绘制只读 [`Palette`]，切换主题就是换一个值。

use gpui_kit::component::theme::Theme;
use gpui_kit::*;
use managi_core::ThemeName;

use crate::i18n::Strings;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Palette {
    pub accent: Rgba,
    pub bg: Rgba,
    pub panel_bg: Rgba,
    pub surface: Rgba,
    pub input_bg: Rgba,
    pub hover_bg: Rgba,
    pub selected_bg: Rgba,
    pub border: Rgba,
    pub border_subtle: Rgba,
    pub font_1: Rgba,
    pub font_2: Rgba,
    pub font_3: Rgba,
    pub green: Rgba,
    pub red: Rgba,
    pub orange: Rgba,
    pub yellow: Rgba,
    pub tab_bar_bg: Rgba,
    pub tab_active_bg: Rgba,
    pub tab_inactive_bg: Rgba,
    pub tab_hover_bg: Rgba,
    pub terminal_bg: Rgba,
    pub terminal_fg: Rgba,
}

impl Palette {
    /// 深色主题用于挑选图标着色策略（深色底用亮图标）。
    pub(crate) fn is_dark(self) -> bool {
        self.bg.r + self.bg.g + self.bg.b < 1.5
    }
}

pub(crate) fn palette(theme: ThemeName) -> Palette {
    match theme {
        ThemeName::Nord => nord_dark(),
        ThemeName::NordLight => nord_light(),
        ThemeName::GithubDark => github_dark(),
        ThemeName::GithubLight => github_light(),
    }
}

fn nord_dark() -> Palette {
    Palette {
        accent: rgb(0x88C0D0),
        bg: rgb(0x2E3440),
        panel_bg: rgb(0x3B4252),
        surface: rgb(0x434C5E),
        input_bg: rgb(0x2E3440),
        hover_bg: rgba(0x88C0D01F),
        selected_bg: rgba(0x88C0D02E),
        border: rgb(0x4C566A),
        border_subtle: rgb(0x434C5E),
        font_1: rgb(0xD8DEE9),
        font_2: rgb(0xE5E9F0),
        font_3: rgb(0x81A1C1),
        green: rgb(0xA3BE8C),
        red: rgb(0xBF616A),
        orange: rgb(0xD08770),
        yellow: rgb(0xEBCB8B),
        tab_bar_bg: rgb(0x2E3440),
        tab_active_bg: rgb(0x3B4252),
        tab_inactive_bg: rgb(0x2E3440),
        tab_hover_bg: rgb(0x434C5E),
        terminal_bg: rgb(0x2E3440),
        terminal_fg: rgb(0xD8DEE9),
    }
}

fn nord_light() -> Palette {
    Palette {
        accent: rgb(0x5E81AC),
        bg: rgb(0xECEFF4),
        panel_bg: rgb(0xE5E9F0),
        surface: rgb(0xFFFFFF),
        input_bg: rgb(0xFFFFFF),
        hover_bg: rgba(0x5E81AC1F),
        selected_bg: rgba(0x5E81AC2E),
        border: rgb(0xD8DEE9),
        border_subtle: rgb(0xE5E9F0),
        font_1: rgb(0x2E3440),
        font_2: rgb(0x3B4252),
        font_3: rgb(0x4C566A),
        green: rgb(0xA3BE8C),
        red: rgb(0xBF616A),
        orange: rgb(0xD08770),
        yellow: rgb(0xEBCB8B),
        tab_bar_bg: rgb(0xECEFF4),
        tab_active_bg: rgb(0xE5E9F0),
        tab_inactive_bg: rgb(0xECEFF4),
        tab_hover_bg: rgb(0xD8DEE9),
        terminal_bg: rgb(0xECEFF4),
        terminal_fg: rgb(0x2E3440),
    }
}

fn github_dark() -> Palette {
    Palette {
        accent: rgb(0x58A6FF),
        bg: rgb(0x0D1117),
        panel_bg: rgb(0x161B22),
        surface: rgb(0x161B22),
        input_bg: rgb(0x0D1117),
        hover_bg: rgba(0x58A6FF1F),
        selected_bg: rgba(0x58A6FF2E),
        border: rgb(0x30363D),
        border_subtle: rgb(0x161B22),
        font_1: rgb(0xC9D1D9),
        font_2: rgb(0xE6EDF3),
        font_3: rgb(0x8B949E),
        green: rgb(0x3FB950),
        red: rgb(0xF85149),
        orange: rgb(0xFB8500),
        yellow: rgb(0xD29922),
        tab_bar_bg: rgb(0x0D1117),
        tab_active_bg: rgb(0x161B22),
        tab_inactive_bg: rgb(0x0D1117),
        tab_hover_bg: rgb(0x30363D),
        terminal_bg: rgb(0x0D1117),
        terminal_fg: rgb(0xC9D1D9),
    }
}

fn github_light() -> Palette {
    Palette {
        accent: rgb(0x0969DA),
        bg: rgb(0xFFFFFF),
        panel_bg: rgb(0xF6F8FA),
        surface: rgb(0xFFFFFF),
        input_bg: rgb(0xFFFFFF),
        hover_bg: rgba(0x0969DA14),
        selected_bg: rgba(0x0969DA1F),
        border: rgb(0xD0D7DE),
        border_subtle: rgb(0xEAEEF2),
        font_1: rgb(0x1F2328),
        font_2: rgb(0x24292F),
        font_3: rgb(0x6E7781),
        green: rgb(0x1A7F37),
        red: rgb(0xCF222E),
        orange: rgb(0xBC4C00),
        yellow: rgb(0x9A6700),
        tab_bar_bg: rgb(0xFFFFFF),
        tab_active_bg: rgb(0xF6F8FA),
        tab_inactive_bg: rgb(0xFFFFFF),
        tab_hover_bg: rgb(0xEAEEF2),
        terminal_bg: rgb(0xFFFFFF),
        terminal_fg: rgb(0x1F2328),
    }
}

/// 把视觉 token 同步进 gpui-component 的全局主题，保证 Input 的直角、字号、
/// 前景/占位符颜色与自绘部分一致。
pub(crate) fn apply_to_component(cx: &mut App, p: Palette, mono_size: u16) {
    Theme::update(cx, |theme| {
        theme.radius = px(0.0);
        theme.radius_lg = px(0.0);
        theme.window_radius = px(0.0);
        theme.shadow = false;
        theme.focus_ring = false;
        theme.font_size = px(12.8);
        theme.mono_font_size = px(f32::from(mono_size));

        theme.colors.background = p.bg.into();
        theme.colors.foreground = p.font_1.into();
        theme.colors.border = p.border.into();
        theme.colors.input = p.input_bg.into();
        theme.colors.caret = p.accent.into();
        theme.colors.selection = p.selected_bg.into();
        theme.colors.muted = p.surface.into();
        theme.colors.muted_foreground = p.font_3.into();
        theme.colors.primary = p.accent.into();
        theme.colors.primary_foreground = p.bg.into();
        theme.colors.accent = p.selected_bg.into();
        theme.colors.accent_foreground = p.accent.into();
        theme.colors.ring = p.accent.into();
        theme.colors.popover = p.panel_bg.into();
        theme.colors.popover_foreground = p.font_1.into();
        theme.colors.secondary = p.surface.into();
        theme.colors.secondary_foreground = p.font_1.into();
        theme.colors.danger = p.red.into();
        theme.colors.danger_foreground = p.bg.into();
        theme.colors.list = p.bg.into();
        theme.colors.list_hover = p.hover_bg.into();
        theme.colors.list_active = p.selected_bg.into();
        theme.colors.sidebar = p.bg.into();
        theme.colors.sidebar_foreground = p.font_1.into();
        theme.colors.tab_bar = p.tab_bar_bg.into();
        theme.colors.tab = p.tab_inactive_bg.into();
        theme.colors.tab_active = p.tab_active_bg.into();
        theme.colors.tab_foreground = p.font_3.into();
        theme.colors.tab_active_foreground = p.font_1.into();
    });
}

/// 界面字体：系统 UI 字体 + 平台 CJK 回退。
pub(crate) fn ui_font() -> Font {
    let fallbacks = if cfg!(target_os = "windows") {
        vec!["Microsoft YaHei UI".to_string(), "Segoe UI".to_string()]
    } else if cfg!(target_os = "macos") {
        vec!["PingFang SC".to_string()]
    } else {
        vec!["Noto Sans CJK SC".to_string(), "Noto Sans".to_string()]
    };

    Font {
        fallbacks: Some(FontFallbacks::from_fonts(fallbacks)),
        ..Font::default()
    }
}

/// 终端占位文字使用的等宽字体；找不到时由 GPUI 回退到系统 UI 字体。
pub(crate) fn mono_font() -> Font {
    let family = if cfg!(target_os = "windows") {
        "Consolas"
    } else if cfg!(target_os = "macos") {
        "Menlo"
    } else {
        "DejaVu Sans Mono"
    };
    let cjk = if cfg!(target_os = "windows") {
        "Microsoft YaHei UI"
    } else if cfg!(target_os = "macos") {
        "PingFang SC"
    } else {
        "Noto Sans CJK SC"
    };

    Font {
        family: family.into(),
        fallbacks: Some(FontFallbacks::from_fonts(vec![cjk.to_string()])),
        ..Font::default()
    }
}

pub(crate) fn theme_label(theme: ThemeName, _strings: Strings) -> &'static str {
    match theme {
        ThemeName::Nord => "Nord Dark",
        ThemeName::NordLight => "Nord Light",
        ThemeName::GithubDark => "GitHub Dark",
        ThemeName::GithubLight => "GitHub Light",
    }
}
