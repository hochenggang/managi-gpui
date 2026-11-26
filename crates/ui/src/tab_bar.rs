//! 标签栏：终端 / 文件管理 / 批量 / 设置四类标签，激活态用 2px accent 下划线。

use gpui_kit::*;

use crate::app_view::{AppView, BAR_HEIGHT};
use crate::assets::icons;
use crate::i18n::Strings;
use crate::state::{AppState, TabKind};
use crate::theme::Palette;
use crate::widgets::icon;

pub(crate) fn render(
    state: &AppState,
    strings: &Strings,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let mut strip = div()
        .id("tab-strip")
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .flex()
        .items_center()
        .overflow_x_scroll()
        .scrollbar_width(px(0.0));

    for tab in &state.tabs {
        let active = state.active_tab.as_deref() == Some(tab.id.as_str());
        let title = state.tab_title(tab, strings);
        let tab_id = tab.id.clone();
        let close_id = tab.id.clone();
        let menu_id = tab.id.clone();

        let tab_icon = match tab.kind {
            TabKind::Terminal => icons::TERMINAL,
            TabKind::Sftp => icons::FOLDER,
            TabKind::Settings => icons::SETTING,
            TabKind::Batch => icons::BATCH,
        };

        let fg = if active { palette.font_1 } else { palette.font_3 };

        strip = strip.child(
            div()
                .relative()
                .h(px(BAR_HEIGHT - 2.0))
                .mt(px(2.0))
                .px(px(12.0))
                .flex_shrink_0()
                .min_w(px(96.0))
                .max_w(px(224.0))
                .flex()
                .items_center()
                .gap(px(5.6))
                .bg(if active {
                    palette.tab_active_bg
                } else {
                    palette.tab_inactive_bg
                })
                .text_color(fg)
                .text_size(px(11.2))
                .border_r_1()
                .border_color(palette.border)
                .cursor_pointer()
                .hover(move |style| style.bg(palette.tab_hover_bg).text_color(palette.font_1))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.state.active_tab = Some(tab_id.clone());
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event, window, cx| {
                        let items = this.tab_menu(&menu_id);
                        this.open_menu(
                            event.position.x.as_f32(),
                            event.position.y.as_f32(),
                            items,
                            window,
                            cx,
                        );
                    }),
                )
                .child(icon(tab_icon, 16.0, fg))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_ellipsis()
                        .child(title),
                )
                .child(
                    div()
                        .w(px(16.0))
                        .h(px(16.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(14.0))
                        .text_color(palette.font_3)
                        .cursor_pointer()
                        .hover(move |style| style.text_color(palette.red))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.state.close_tab(&close_id);
                            cx.notify();
                        }))
                        .child("×"),
                )
                .when(active, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left(px(0.0))
                            .right(px(0.0))
                            .bottom(px(0.0))
                            .h(px(2.0))
                            .bg(palette.accent),
                    )
                }),
        );
    }

    strip = strip.child(
        div()
            .w(px(28.0))
            .h(px(28.0))
            .ml(px(4.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |style| style.bg(palette.hover_bg))
            .on_click(cx.listener(|this, _, _, cx| {
                this.state.open_singleton(TabKind::Batch);
                cx.notify();
            }))
            .child(icon(icons::PLUS, 16.0, palette.font_2)),
    );

    div()
        .h(px(BAR_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .bg(palette.tab_bar_bg)
        .border_b_1()
        .border_color(palette.border)
        .child(strip)
        .child(div().w(px(8.0)).flex_shrink_0())
}
