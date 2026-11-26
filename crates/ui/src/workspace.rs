//! 工作区：按激活标签分派到终端、SFTP、批量执行或设置页。

use std::collections::HashMap;

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::app_view::AppView;
use crate::assets::icons;
use crate::batch_runtime::BatchRuntime;
use crate::i18n::Strings;
use crate::sftp_runtime::SftpRuntime;
use crate::state::{AppState, TabKind};
use crate::terminal_runtime::TerminalRuntime;
use crate::theme::Palette;
use crate::widgets::icon;

pub(crate) fn render(
    state: &AppState,
    strings: &Strings,
    palette: Palette,
    terminals: &HashMap<String, TerminalRuntime>,
    sftp: &HashMap<String, SftpRuntime>,
    batches: &HashMap<String, BatchRuntime>,
    batch_command: &Entity<InputState>,
    focus: &FocusHandle,
    quick_open: bool,
    window: &Window,
    cx: &mut Context<AppView>,
) -> Div {
    let body = match state.active_tab() {
        None => empty_state(strings, palette),
        Some(tab) => match tab.kind {
            TabKind::Terminal => crate::terminal_view::render(
                terminals.get(&tab.id),
                tab.node.as_ref(),
                strings,
                palette,
                f32::from(state.config.settings.terminal_font_size),
                state.config.settings.language,
                quick_open,
                focus,
                cx,
            ),
            TabKind::Sftp => crate::sftp_view::render(
                sftp.get(&tab.id),
                tab.node.as_ref(),
                strings,
                palette,
                cx,
            ),
            TabKind::Batch => crate::batch_view::render(
                batches.get(&tab.id),
                batch_command,
                strings,
                palette,
                window,
                cx,
            ),
            TabKind::Settings => crate::settings_view::render(state, strings, palette, cx),
        },
    };

    div()
        .flex_1()
        .flex()
        .flex_col()
        .min_w(px(0.0))
        .child(body)
}

fn empty_state(strings: &Strings, palette: Palette) -> Div {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .bg(palette.bg)
        .child(icon(icons::TERMINAL, 40.0, palette.font_3))
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.font_1)
                .child("Managi"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(palette.font_3)
                .child(strings.empty_hint.to_string()),
        )
}
