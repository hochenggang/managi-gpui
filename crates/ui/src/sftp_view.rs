//! SFTP 文件管理：路径栏、文件表、行内动作与传输状态。

use gpui_kit::prelude::*;
use gpui_kit::*;
use managi_core::Node;
use managi_ssh::{format_mtime, human_size};

use crate::app_view::{AppView, BAR_HEIGHT};
use crate::i18n::Strings;
use crate::sftp_runtime::SftpRuntime;
use crate::theme::{self, Palette};

pub(crate) fn render(
    runtime: Option<&SftpRuntime>,
    node: Option<&Node>,
    strings: &Strings,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> Div {
    let mut root = div()
        .flex_1()
        .flex()
        .flex_col()
        .min_h(px(0.0))
        .bg(palette.panel_bg);

    let path = runtime.map(|runtime| runtime.path.clone()).unwrap_or_default();
    let mut toolbar = div()
        .h(px(40.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .border_b_1()
        .border_color(palette.border)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .h(px(26.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .border_1()
                .border_color(palette.border)
                .bg(palette.bg)
                .text_size(px(11.2))
                .text_color(palette.font_2)
                .text_ellipsis()
                .child(if path.is_empty() {
                    "/".to_string()
                } else {
                    path.clone()
                }),
        );

    let live = runtime.is_some_and(|runtime| runtime.connected);
    toolbar = toolbar.child(
        tool_button(
            "sftp-up",
            strings.sftp_up,
            palette,
            live,
            cx.listener(|this, _, _, cx| {
                this.sftp_up(cx);
            }),
        )
        .into_any_element(),
    );
    toolbar = toolbar.child(
        tool_button(
            "sftp-refresh",
            strings.sftp_refresh,
            palette,
            live,
            cx.listener(|this, _, _, cx| {
                this.sftp_refresh(cx);
            }),
        )
        .into_any_element(),
    );
    toolbar = toolbar.child(
        tool_button(
            "sftp-mkdir",
            strings.sftp_new_dir,
            palette,
            live,
            cx.listener(|this, _, window, cx| {
                this.sftp_new_dir(window, cx);
            }),
        )
        .into_any_element(),
    );
    toolbar = toolbar.child(
        tool_button(
            "sftp-upload",
            strings.sftp_upload,
            palette,
            live,
            cx.listener(|this, _, window, cx| {
                this.sftp_upload(window, cx);
            }),
        )
        .into_any_element(),
    );

    let selected_is_file = runtime.is_some_and(|runtime| {
        let Some(name) = runtime.selected.as_deref() else {
            return false;
        };
        runtime
            .entries
            .iter()
            .any(|entry| entry.name == name && !entry.is_dir)
    });
    toolbar = toolbar.child(
        tool_button(
            "sftp-download",
            strings.sftp_download,
            palette,
            live && selected_is_file,
            cx.listener(|this, _, _, cx| {
                this.sftp_download(cx);
            }),
        )
        .into_any_element(),
    );
    toolbar = toolbar.child(
        tool_button(
            "sftp-delete",
            strings.sftp_delete,
            palette,
            live,
            cx.listener(|this, _, _, cx| {
                this.sftp_delete(cx);
            }),
        )
        .into_any_element(),
    );
    root = root.child(toolbar);

    let mut list = div()
        .id("sftp-list")
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .scrollbar_width(px(0.0))
        .font(theme::ui_font());
    list = list.child(
        div()
            .h(px(28.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(12.0))
            .border_b_1()
            .border_color(palette.border)
            .text_size(px(10.4))
            .text_color(palette.font_3)
            .child(div().flex_1().child(strings.sftp_name.to_string()))
            .child(
                div()
                    .w(px(92.0))
                    .text_right()
                    .child(strings.sftp_size.to_string()),
            )
            .child(
                div()
                    .w(px(132.0))
                    .pl(px(12.0))
                    .child(strings.sftp_modified.to_string()),
            ),
    );

    if let Some(runtime) = runtime {
        if let Some(error) = &runtime.error {
            list = list.child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .text_size(px(11.2))
                    .text_color(palette.red)
                    .child(error.clone()),
            );
        }
        if runtime.loading {
            list = list.child(empty_row(strings.sftp_loading, palette));
        } else if runtime.entries.is_empty() {
            list = list.child(empty_row(strings.sftp_empty, palette));
        } else {
            for (index, entry) in runtime.entries.iter().take(500).enumerate() {
                let selected = runtime.selected.as_deref() == Some(entry.name.as_str());
                let name = entry.name.clone();
                let is_dir = entry.is_dir;
                let background = if selected {
                    palette.selected_bg
                } else {
                    palette.panel_bg
                };
                let row = div()
                    .id(ElementId::named_usize("sftp-row", index))
                    .h(px(30.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(12.0))
                    .bg(background)
                    .border_b_1()
                    .border_color(palette.border)
                    .text_size(px(12.0))
                    .cursor_pointer()
                    .hover(move |style| style.bg(palette.hover_bg))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sftp_select(&name, cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .text_color(if is_dir {
                                palette.accent
                            } else {
                                palette.font_1
                            })
                            .child(if is_dir { "▸ " } else { "  " })
                            .child(div().text_ellipsis().child(entry.name.clone())),
                    )
                    .child(
                        div()
                            .w(px(92.0))
                            .text_right()
                            .text_color(palette.font_3)
                            .child(if is_dir {
                                "-".to_string()
                            } else {
                                human_size(entry.size)
                            }),
                    )
                    .child(
                        div()
                            .w(px(132.0))
                            .pl(px(12.0))
                            .text_color(palette.font_3)
                            .child(format_mtime(entry.modified)),
                    );

                list = list.child(row);
            }
        }
    } else {
        list = list.child(empty_row(
            if node.is_some() {
                strings.connecting
            } else {
                strings.disconnected
            },
            palette,
        ));
    }
    root = root.child(list);

    let info = match runtime {
        Some(runtime) => format!(
            "{} {}  {}",
            runtime.entries.len(),
            strings.sftp_name,
            node.map(|node| format!("{} ({}:{})", node.name, node.host, node.port))
                .unwrap_or_default()
        ),
        None => node
            .map(|node| format!("{} ({}:{})", node.name, node.host, node.port))
            .unwrap_or_else(|| strings.idle.to_string()),
    };
    let status = match runtime {
        Some(runtime) if runtime.connected => {
            if let Some((label, percent)) = &runtime.progress {
                (format!("{label} {percent}%"), palette.yellow)
            } else {
                (strings.sftp_refresh.to_string(), palette.green)
            }
        }
        Some(runtime) => (
            runtime
                .error
                .clone()
                .unwrap_or_else(|| strings.disconnected.to_string()),
            palette.red,
        ),
        None => (strings.connecting.to_string(), palette.yellow),
    };

    root.child(
        div()
            .h(px(BAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .px(px(12.0))
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
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .max_w(px(520.0))
                    .text_color(status.1)
                    .child(
                        div()
                            .w(px(4.8))
                            .h(px(4.8))
                            .rounded(px(2.4))
                            .bg(status.1),
                    )
                    .child(div().text_ellipsis().child(status.0)),
            ),
    )
}

fn empty_row(message: &'static str, palette: Palette) -> Div {
    div()
        .h(px(120.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.2))
        .text_color(palette.font_3)
        .child(message.to_string())
}

fn tool_button(
    id: &'static str,
    label: &'static str,
    palette: Palette,
    enabled: bool,
    listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let mut button = div()
        .id(id)
        .h(px(26.0))
        .flex_shrink_0()
        .px(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(palette.border)
        .text_size(px(11.2))
        .child(label.to_string());

    if enabled {
        button = button
            .cursor_pointer()
            .text_color(palette.font_1)
            .hover(move |style| style.border_color(palette.accent).text_color(palette.accent))
            .on_click(listener);
    } else {
        button = button.text_color(palette.font_3);
    }
    button
}
