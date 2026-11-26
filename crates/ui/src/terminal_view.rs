//! 终端网格绘制：alacritty 快照 → GPUI 行/颜色段 + 光标 + 底栏。

use gpui_kit::prelude::*;
use gpui_kit::*;
use managi_core::Node;
use managi_term::{SnapshotColor, SnapshotRow, SnapshotRun};

use crate::app_view::{AppView, BAR_HEIGHT};
use crate::i18n::Strings;
use crate::terminal_runtime::{TerminalRuntime, TerminalStatus};
use crate::theme::{self, Palette};

pub(crate) fn render(
    runtime: Option<&TerminalRuntime>,
    node: Option<&Node>,
    strings: &Strings,
    palette: Palette,
    font_size: f32,
    focus: &FocusHandle,
    cx: &mut Context<AppView>,
) -> Div {
    let line_height = (font_size * 1.28).max(12.0);
    let cell_width = (font_size * 0.62).max(6.0);

    let mut screen = div()
        .relative()
        .flex_1()
        .min_h(px(0.0))
        .overflow_hidden()
        .bg(palette.terminal_bg)
        .p(px(8.0))
        .font(theme::mono_font())
        .text_size(px(font_size));

    if let Some(runtime) = runtime {
        let snapshot = runtime.model.snapshot();
        let mut rows = div().flex().flex_col().w_full();
        for row in &snapshot.rows {
            rows = rows.child(row_element(row, palette, font_size, line_height));
        }
        screen = screen.child(rows);

        if snapshot.cursor.visible {
            screen = screen.child(
                div()
                    .absolute()
                    .left(px(8.0 + snapshot.cursor.column as f32 * cell_width))
                    .top(px(8.0 + snapshot.cursor.line as f32 * line_height))
                    .w(px(2.0))
                    .h(px(line_height))
                    .bg(palette.accent),
            );
        }

        if matches!(runtime.status, TerminalStatus::Connecting) {
            screen = screen.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .w_full()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(palette.terminal_bg)
                    .text_size(px(11.2))
                    .text_color(palette.font_3)
                    .child(strings.connecting.to_string()),
            );
        } else if let TerminalStatus::Error(error) = &runtime.status {
            screen = screen.child(
                div()
                    .absolute()
                    .left(px(16.0))
                    .top(px(12.0))
                    .max_w(px(620.0))
                    .text_size(px(12.0))
                    .text_color(palette.red)
                    .child(error.clone()),
            );
        }
    } else {
        screen = screen
            .child(
                div()
                    .font(theme::mono_font())
                    .text_size(px(font_size))
                    .text_color(palette.terminal_fg)
                    .child(
                        node.map(|node| {
                            format!("$ ssh {}@{} -p {}", node.username, node.host, node.port)
                        })
                        .unwrap_or_else(|| strings.disconnected.to_string()),
                    ),
            )
            .child(
                div()
                    .mt(px(8.0))
                    .text_size(px(11.2))
                    .text_color(palette.font_3)
                    .child(strings.disconnected.to_string()),
            );
    }

    let info = node
        .map(|node| format!("{} ({}:{})", node.name, node.host, node.port))
        .unwrap_or_else(|| strings.idle.to_string());

    let status = runtime
        .map(|runtime| match &runtime.status {
            TerminalStatus::Connecting => (strings.connecting.to_string(), palette.yellow),
            TerminalStatus::Connected => (strings.connected.to_string(), palette.green),
            TerminalStatus::Closed(reason) => {
                (format!("{} · {reason}", strings.disconnected), palette.red)
            }
            TerminalStatus::Error(error) => (error.clone(), palette.red),
        })
        .unwrap_or_else(|| (strings.idle.to_string(), palette.font_3));
    let needs_reconnect =
        runtime.is_some_and(|runtime| !runtime.is_live()) || (runtime.is_none() && node.is_some());

    let mut toolbar = div()
        .h(px(BAR_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
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

    if needs_reconnect {
        toolbar = toolbar.child(
            div()
                .id("terminal-reconnect")
                .flex_shrink_0()
                .h(px(22.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .cursor_pointer()
                .border_1()
                .border_color(palette.border)
                .text_color(palette.accent)
                .hover(move |style| style.bg(palette.hover_bg))
                .on_click(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.reconnect_active(window, cx);
                }))
                .child(strings.reconnect.to_string()),
        );
    }

    toolbar = toolbar.child(
        div()
            .flex_shrink_0()
            .max_w(px(560.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .text_size(px(12.0))
            .text_color(status.1)
            .child(
                div()
                    .w(px(4.8))
                    .h(px(4.8))
                    .rounded(px(2.4))
                    .bg(status.1),
            )
            .child(div().text_ellipsis().child(status.0)),
    );

    let body = if runtime.is_some() {
        screen
            .track_focus(focus)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                this.term_focus.focus(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.handle_terminal_key(event, cx);
            }))
    } else {
        screen
    };

    div()
        .flex_1()
        .flex()
        .flex_col()
        .min_h(px(0.0))
        .bg(palette.terminal_bg)
        .child(body)
        .child(toolbar)
}

fn row_element(row: &SnapshotRow, palette: Palette, font_size: f32, line_height: f32) -> Div {
    let mut text = String::new();
    let mut runs = Vec::with_capacity(row.runs.len());

    for run in &row.runs {
        text.push_str(&run.text);
        if run.text.is_empty() {
            continue;
        }
        runs.push(text_run(run, text.len(), palette));
    }

    if text.is_empty() {
        text.push(' ');
    }

    div()
        .h(px(line_height))
        .w_full()
        .flex_shrink_0()
        .overflow_hidden()
        .child(StyledText::new(text).with_runs(runs).text_size(px(font_size)))
}

fn text_run(run: &SnapshotRun, end: usize, palette: Palette) -> TextRun {
    let start = end.saturating_sub(run.text.len());
    let mut font = theme::mono_font();
    if run.bold {
        font.weight = FontWeight::SEMIBOLD;
    }
    TextRun {
        len: end - start,
        font,
        color: resolve_color(run.fg, palette).into(),
        background_color: Some(resolve_color(run.bg, palette).into()),
        underline: None,
        strikethrough: None,
    }
}

fn resolve_color(color: SnapshotColor, palette: Palette) -> Rgba {
    match color {
        SnapshotColor::Rgb(r, g, b) => rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)),
        SnapshotColor::Index(index) => indexed_color(index, palette),
    }
}

fn indexed_color(index: u16, palette: Palette) -> Rgba {
    match index {
        0 => rgb(0x3B4252),
        1 => palette.red,
        2 => palette.green,
        3 => palette.yellow,
        4 => palette.accent,
        5 => rgb(0xB48EAD),
        6 => palette.accent,
        7 => palette.font_1,
        8 => palette.font_3,
        9 => palette.red,
        10 => palette.green,
        11 => palette.yellow,
        12 => palette.accent,
        13 => rgb(0xB48EAD),
        14 => palette.accent,
        15 => rgb(0xECEFF4),
        16..=231 => {
            let value = index - 16;
            let r = (value / 36) % 6;
            let g = (value / 6) % 6;
            let b = value % 6;
            let level = |slot: u16| -> u8 {
                match slot {
                    0 => 0,
                    1 => 95,
                    2 => 135,
                    3 => 175,
                    4 => 215,
                    _ => 255,
                }
            };
            rgb(u32::from(level(r)) << 16 | u32::from(level(g)) << 8 | u32::from(level(b)))
        }
        232..=255 => {
            let shade = 8 + (index - 232) as u8 * 10;
            rgb(u32::from(shade) << 16 | u32::from(shade) << 8 | u32::from(shade))
        }
        256 => palette.terminal_fg,
        257 => palette.terminal_bg,
        258 => palette.accent,
        _ => palette.terminal_fg,
    }
}
