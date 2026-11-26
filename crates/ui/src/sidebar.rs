//! 侧栏：搜索、分组树、节点行动作、折叠与设置入口。

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::app_view::{sidebar_width, AppView};
use crate::assets::icons;
use crate::i18n::Strings;
use crate::state::{AppState, TabKind};
use crate::theme::Palette;
use crate::widgets::{icon, text_input};

pub(crate) fn render(
    state: &AppState,
    search: &Entity<InputState>,
    strings: &Strings,
    palette: Palette,
    window: &Window,
    cx: &mut Context<AppView>,
) -> Div {
    let width = sidebar_width(state.sidebar_collapsed);
    let groups = state.display_groups(strings);

    let mut list = div()
        .id("sidebar-groups")
        .flex_1()
        .overflow_y_scroll()
        .scrollbar_width(px(0.0))
        .p(px(8.0));

    for group in groups {
        list = list.child(group_block(state, &group, palette, cx));
    }

    div()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .bg(palette.bg)
        .border_r_1()
        .border_color(palette.border)
        .child(
            div()
                .h(px(32.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(5.6))
                .border_b_1()
                .border_color(palette.border)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(text_input(search, palette, 32.0, 11.2, window, cx)),
                )
                .child(
                    div()
                        .id("sidebar-add-node")
                        .w(px(28.0))
                        .h(px(28.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(move |style| style.bg(palette.hover_bg))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_node_form(None, window, cx);
                        }))
                        .child(icon(icons::PLUS, 14.0, palette.font_2)),
                ),
        )
        .child(list)
        .child(footer(state, palette, cx))
}

fn group_block(
    state: &AppState,
    group: &crate::state::DisplayGroup,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let key = if group.all_hosts {
        String::new()
    } else {
        group.label.clone()
    };
    let node_ids = state.node_ids(&key);
    let selected = state.is_group_selected(&node_ids);
    let partial = state.is_group_partial(&node_ids);
    let collapsed = state.is_group_collapsed(&group.label);

    let ids_for_click = node_ids.clone();
    let menu_key = key.clone();
    let chevron_key = group.label.clone();
    let header_id = format!("sidebar-group-{}", group.label);
    let chevron_id = format!("sidebar-group-chevron-{}", group.label);

    let header = div()
        .id(header_id)
        .flex()
        .items_center()
        .py(px(7.2))
        .px(px(8.0))
        .cursor_pointer()
        .text_size(px(11.2))
        .text_color(if selected || partial {
            palette.accent
        } else if group.all_hosts {
            palette.font_1
        } else {
            palette.font_2
        })
        .when(selected, |el| el.bg(palette.selected_bg))
        .hover(move |style| style.bg(palette.hover_bg))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.state.toggle_group(&ids_for_click);
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event, window, cx| {
                let items = this.group_menu(&menu_key);
                this.open_menu(
                    event.position.x.as_f32(),
                    event.position.y.as_f32(),
                    items,
                    window,
                    cx,
                );
            }),
        )
        .child(
            div()
                .id(chevron_id)
                .w(px(16.0))
                .h(px(16.0))
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.state.toggle_collapsed(&chevron_key);
                    cx.notify();
                }))
                .child(icon(
                    if collapsed {
                        icons::CHEVRON_RIGHT
                    } else {
                        icons::CHEVRON_DOWN
                    },
                    12.0,
                    palette.font_3,
                )),
        )
        .child(
            div()
                .ml(px(4.0))
                .flex_1()
                .min_w(px(0.0))
                .text_ellipsis()
                .when(group.all_hosts, |el| el.font_weight(FontWeight::SEMIBOLD))
                .child(group.label.clone()),
        )
        .child(
            div()
                .pl(px(8.0))
                .text_color(palette.font_3)
                .child(group.count.to_string()),
        );

    let mut nodes = div().flex().flex_col().pl(px(24.0));
    if !collapsed {
        for node in &group.nodes {
            nodes = nodes.child(node_row(state, node, palette, cx));
        }
    }

    div()
        .flex()
        .flex_col()
        .mb(px(4.0))
        .child(header)
        .child(nodes)
}

fn node_row(
    state: &AppState,
    node: &managi_core::Node,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let node_id = node.id.clone();
    let selected = state.is_selected(&node_id);
    let group_name = SharedString::from(format!("node-row-{}", node.id));
    let term_id = format!("node-term-{}", node.id);
    let sftp_id = format!("node-folder-{}", node.id);

    let terminal_node = node.clone();
    let sftp_node = node.clone();
    let menu_node = node.id.clone();

    let row = div()
        .id(format!("node-row-{}", node.id))
        .relative()
        .w_full()
        .flex()
        .items_center()
        .gap(px(5.6))
        .py(px(5.6))
        .cursor_pointer()
        .text_size(px(11.2))
        .text_color(if selected {
            palette.accent
        } else {
            palette.font_2
        })
        .group(group_name.clone())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.state.toggle_node(&node_id);
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event, window, cx| {
                let items = this.node_menu(&menu_node);
                this.open_menu(
                    event.position.x.as_f32(),
                    event.position.y.as_f32(),
                    items,
                    window,
                    cx,
                );
            }),
        )
        .when(selected, |el| {
            el.child(
                div()
                    .absolute()
                    .left(px(-12.0))
                    .top(px(0.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .w(px(4.8))
                            .h(px(4.8))
                            .rounded(px(2.4))
                            .bg(palette.green),
                    ),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_ellipsis()
                .child(node.name.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                // 动作图标只在悬停行时出现，和 v3 的 NodeList 一致。
                .invisible()
                .group_hover(group_name, |style| style.visible())
                .child(
                    div()
                        .id(term_id)
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(move |style| style.bg(palette.hover_bg))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.state.open_node_tab(TabKind::Terminal, &terminal_node);
                            cx.notify();
                        }))
                        .child(icon(icons::TERMINAL, 13.6, palette.font_3)),
                )
                .child(
                    div()
                        .id(sftp_id)
                        .w(px(18.0))
                        .h(px(18.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(move |style| style.bg(palette.hover_bg))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.state.open_node_tab(TabKind::Sftp, &sftp_node);
                            cx.notify();
                        }))
                        .child(icon(icons::FOLDER, 13.6, palette.font_3)),
                ),
        );

    row
}

fn footer(state: &AppState, palette: Palette, cx: &mut Context<AppView>) -> impl IntoElement {
    let collapsed = state.sidebar_collapsed;
    let chevron = if collapsed {
        icons::CHEVRON_RIGHT
    } else {
        icons::CHEVRON_LEFT
    };

    div()
        .h(px(32.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .px(px(12.0))
        .border_t_1()
        .border_color(palette.border)
        .child(
            div()
                .id("sidebar-collapse")
                .w(px(28.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(move |style| style.bg(palette.hover_bg))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.state.sidebar_collapsed = !this.state.sidebar_collapsed;
                    cx.notify();
                }))
                .child(icon(chevron, 16.0, palette.font_2)),
        )
        .child(
            div()
                .id("sidebar-settings")
                .w(px(28.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(move |style| style.bg(palette.hover_bg))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.state.open_singleton(TabKind::Settings);
                    cx.notify();
                }))
                .child(icon(icons::SETTING, 16.0, palette.font_2)),
        )
}
