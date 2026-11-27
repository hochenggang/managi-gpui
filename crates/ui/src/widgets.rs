//! 两个高频小组件：自绘直角输入框外壳、可着色 SVG 图标。

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::Palette;

pub(crate) fn icon(path: &'static str, size: f32, color: Rgba) -> Svg {
    svg()
        .path(path)
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        .text_color(color)
}

pub(crate) fn text_input(
    state: &Entity<InputState>,
    palette: Palette,
    height: f32,
    font_size: f32,
    window: &Window,
    cx: &App,
) -> Div {
    text_input_impl(state, palette, height, font_size, window, cx, true)
}

/// 侧栏搜索框：无边框，只保留输入区和底色。
pub(crate) fn text_input_plain(
    state: &Entity<InputState>,
    palette: Palette,
    height: f32,
    font_size: f32,
    window: &Window,
    cx: &App,
) -> Div {
    text_input_impl(state, palette, height, font_size, window, cx, false)
}

fn text_input_impl(
    state: &Entity<InputState>,
    palette: Palette,
    height: f32,
    font_size: f32,
    window: &Window,
    cx: &App,
    bordered: bool,
) -> Div {
    let focused = state.read(cx).focus_handle(cx).is_focused(window);
    let border = if focused {
        palette.accent
    } else {
        palette.border
    };

    let mut input = div()
        .h(px(height))
        .flex()
        .items_center()
        .bg(palette.input_bg);
    if bordered {
        input = input.border_1().border_color(border);
    }
    input.child(
        Input::new(state)
            .appearance(false)
            .bordered(false)
            .text_size(px(font_size))
            .text_color(palette.font_1)
            .w_full(),
    )
}
