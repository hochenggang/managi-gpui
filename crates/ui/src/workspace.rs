//! 工作区：按激活标签分派到终端占位、SFTP 占位、批量占位或设置页。
//!
//! M1 只画外壳；真正的 SSH/PTY 在 M2 接入，因此终端页明确标注当前状态。

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::app_view::{AppView, BAR_HEIGHT};
use crate::assets::icons;
use crate::i18n::Strings;
use crate::state::{AppState, TabKind};
use crate::theme::{self, Palette};
use crate::widgets::icon;

pub(crate) fn render(
    state: &AppState,
    strings: &Strings,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> Div {
    let body = match state.active_tab() {
        None => empty_state(strings, palette),
        Some(tab) => match tab.kind {
            TabKind::Terminal => terminal_panel(tab.node.as_ref(), strings, palette),
            TabKind::Sftp => notice_panel(strings.sftp_notice, palette),
            TabKind::Batch => notice_panel(strings.batch_notice, palette),
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

fn notice_panel(message: &'static str, palette: Palette) -> Div {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .bg(palette.bg)
        .text_size(px(11.2))
        .text_color(palette.font_3)
        .child(message.to_string())
}

fn terminal_panel(node: Option<&managi_core::Node>, strings: &Strings, palette: Palette) -> Div {
    let mut screen = div()
        .flex_1()
        .min_w(px(0.0))
        .p(px(8.0))
        .flex()
        .flex_col()
        .bg(palette.terminal_bg);

    match node {
        Some(node) => {
            screen = screen
                .child(
                    div()
                        .font(theme::mono_font())
                        .text_size(px(14.0))
                        .text_color(palette.terminal_fg)
                        .child(format!(
                            "$ ssh {}@{} -p {}",
                            node.username, node.host, node.port
                        )),
                )
                .child(
                    div()
                        .mt(px(8.0))
                        .font(theme::mono_font())
                        .text_size(px(11.2))
                        .text_color(palette.font_3)
                        .child(strings.m1_notice.to_string()),
                )
                .child(
                    div()
                        .mt(px(8.0))
                        .w(px(8.0))
                        .h(px(16.0))
                        .bg(palette.terminal_fg),
                );
        }
        None => {
            screen = screen.child(
                div()
                    .font(theme::mono_font())
                    .text_size(px(11.2))
                    .text_color(palette.font_3)
                    .child(strings.idle.to_string()),
            );
        }
    }

    let info = node
        .map(|node| format!("{} ({}:{})", node.name, node.host, node.port))
        .unwrap_or_else(|| strings.idle.to_string());

    let mut toolbar = div()
        .h(px(BAR_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .px(px(12.0))
        .bg(palette.panel_bg)
        .border_t_1()
        .border_color(palette.border)
        .text_size(px(11.2))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_ellipsis()
                .text_color(palette.font_2)
                .child(info),
        );

    toolbar = toolbar.child(
        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(4.0))
            .text_size(px(12.0))
            .text_color(palette.font_3)
            .child(
                div()
                    .w(px(4.8))
                    .h(px(4.8))
                    .rounded(px(2.4))
                    .bg(palette.font_3),
            )
            .child(strings.idle.to_string()),
    );

    div()
        .flex_1()
        .flex()
        .flex_col()
        .bg(palette.terminal_bg)
        .child(screen)
        .child(toolbar)
}
