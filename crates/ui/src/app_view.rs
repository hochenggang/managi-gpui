//! M0 的最小壳层：验证 GPUI 能在三平台开窗、并按设计 token 上色。
//!
//! M1 会把这里替换成「侧栏 + 标签栏 + 工作区 + 底栏」的完整布局。

use gpui_kit::*;

use crate::theme;

pub struct AppView {
    version: SharedString,
}

impl AppView {
    pub fn new(version: impl Into<SharedString>) -> Self {
        Self {
            version: version.into(),
        }
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .gap(px(8.0))
            .bg(rgb(theme::BG))
            .text_color(rgb(theme::FONT_1))
            .child(
                div()
                    .text_size(px(theme::TEXT_TITLE))
                    .child("managi-gpui"),
            )
            .child(
                div()
                    .text_size(px(theme::TEXT_BAR))
                    .text_color(rgb(theme::FONT_3))
                    .child(self.version.clone()),
            )
    }
}
