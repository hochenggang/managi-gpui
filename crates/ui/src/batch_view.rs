//! 批量执行：命令输入、逐主机结果卡片、停止。

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::app_view::{AppView, primary_button};
use crate::batch_runtime::{BatchRuntime, HostState};
use crate::i18n::Strings;
use crate::theme::{self, Palette};
use crate::widgets::text_input;

pub(crate) fn render(
    runtime: Option<&BatchRuntime>,
    command: &Entity<InputState>,
    strings: &Strings,
    palette: Palette,
    window: &Window,
    cx: &mut Context<AppView>,
) -> Div {
    let mut root = div()
        .flex_1()
        .flex()
        .flex_col()
        .min_h(px(0.0))
        .bg(palette.bg);

    let running = runtime.is_some_and(|runtime| runtime.running());
    let mut bar = div()
        .h(px(44.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .border_b_1()
        .border_color(palette.border)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .child(text_input(command, palette, 30.0, 12.0, window, cx)),
        )
        .child(primary_button(strings.batch_run, palette).on_click(cx.listener(
            |this, _, window, cx| {
                this.run_batch(window, cx);
            },
        )));

    if running {
        bar = bar.child(
            div()
                .id("batch-stop-all")
                .h(px(30.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .border_1()
                .border_color(palette.red)
                .text_size(px(12.8))
                .text_color(palette.red)
                .cursor_pointer()
                .hover(move |style| style.bg(palette.hover_bg))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.batch_stop_all(cx);
                }))
                .child(strings.batch_stop_all.to_string()),
        );
    }
    root = root.child(bar);

    let body = match runtime {
        None => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.2))
            .text_color(palette.font_3)
            .child(strings.batch_empty.to_string())
            .into_any_element(),
        Some(runtime) => {
            let mut list = div()
                .id("batch-results")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .scrollbar_width(px(0.0))
                .p(px(10.0))
                .flex()
                .flex_col()
                .gap(px(8.0));

            for (index, host) in runtime.hosts.iter().enumerate() {
                list = list.child(host_card(index, host, strings, palette, cx));
            }
            list.into_any_element()
        }
    };
    root = root.child(body);

    root.child(
        div()
            .h(px(32.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .border_t_1()
            .border_color(palette.border)
            .text_size(px(11.2))
            .text_color(palette.font_2)
            .child(format!(
                "{} · {}",
                runtime.map(|runtime| runtime.hosts.len()).unwrap_or(0),
                clip_command(runtime.map(|r| r.command.as_str()).unwrap_or(""), 80)
            ))
            .child(if running {
                strings.batch_running.to_string()
            } else {
                strings.batch_run.to_string()
            }),
    )
}

fn host_card(
    index: usize,
    host: &crate::batch_runtime::HostRun,
    strings: &Strings,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> Div {
    let status = match &host.state {
        HostState::Running => (strings.batch_running.to_string(), palette.yellow),
        HostState::Done { code: None, .. } => ("完成".to_string(), palette.green),
        HostState::Done { code: Some(0), .. } => ("0".to_string(), palette.green),
        HostState::Done { code: Some(code), .. } => (format!("{} {code}", strings.batch_exit), palette.red),
        HostState::Error(message) => (message.clone(), palette.red),
        HostState::Stopped => (strings.batch_stopped.to_string(), palette.font_3),
    };

    let mut header = div()
        .h(px(28.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(10.0))
        .border_b_1()
        .border_color(palette.border)
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.font_1)
                .child(host.name.clone()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(px(10.4))
                .text_color(status.1)
                .text_ellipsis()
                .child(status.0),
        );

    if matches!(host.state, HostState::Running) {
        header = header.child(
            div()
                .id(ElementId::named_usize("batch-stop", index))
                .h(px(20.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .border_1()
                .border_color(palette.border)
                .text_size(px(10.4))
                .text_color(palette.font_2)
                .cursor_pointer()
                .hover(move |style| style.border_color(palette.red).text_color(palette.red))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.batch_stop(index, cx);
                }))
                .child(strings.batch_stop.to_string()),
        );
    }

    let output = match &host.state {
        HostState::Done { output, .. } => {
            if output.trim().is_empty() {
                "(no output)".to_string()
            } else {
                output.clone()
            }
        }
        HostState::Error(message) => message.clone(),
        HostState::Running => strings.batch_running.to_string(),
        HostState::Stopped => strings.batch_stopped.to_string(),
    };

    div()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .border_1()
        .border_color(palette.border)
        .bg(palette.panel_bg)
        .child(header)
        .child(
            div()
                .id(ElementId::named_usize("batch-output", index))
                .h(px(150.0))
                .overflow_y_scroll()
                .scrollbar_width(px(0.0))
                .p(px(10.0))
                .font(theme::mono_font())
                .text_size(px(11.2))
                .text_color(if matches!(host.state, HostState::Error(_)) {
                    palette.red
                } else {
                    palette.font_1
                })
                .child(output),
        )
}

fn clip_command(command: &str, max: usize) -> String {
    if command.chars().count() <= max {
        command.to_string()
    } else {
        let clipped: String = command.chars().take(max).collect();
        format!("{clipped}…")
    }
}
