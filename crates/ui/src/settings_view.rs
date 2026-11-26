//! 设置页：左侧 192px 导航，右侧内容。外观/终端可选，数据与关于为静态说明。

use gpui_kit::prelude::*;
use gpui_kit::*;
use managi_core::settings::{MAX_TERMINAL_FONT_SIZE, MIN_TERMINAL_FONT_SIZE};
use managi_core::{ThemeName};

use crate::app_view::AppView;
use crate::i18n::Strings;
use crate::state::{AppState, SettingsSection};
use crate::theme::{self, Palette};

const NAV_WIDTH: f32 = 192.0;

pub(crate) fn render(
    state: &AppState,
    strings: &Strings,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> Div {
    let mut nav = div()
        .w(px(NAV_WIDTH))
        .flex_shrink_0()
        .h_full()
        .p(px(8.0))
        .flex()
        .flex_col()
        .bg(palette.panel_bg)
        .border_r_1()
        .border_color(palette.border);

    for (index, (section, label)) in [
        (SettingsSection::Appearance, strings.appearance),
        (SettingsSection::Terminal, strings.terminal),
        (SettingsSection::Data, strings.data),
        (SettingsSection::About, strings.about),
    ]
    .into_iter()
    .enumerate()
    {
        let active = state.settings_section == section;
        nav = nav.child(
            div()
                .id(ElementId::named_usize("settings-nav", index))
                .h(px(36.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .mb(px(4.0))
                .cursor_pointer()
                .text_size(px(14.4))
                .text_color(if active {
                    palette.accent
                } else {
                    palette.font_2
                })
                .when(active, |el| el.bg(palette.hover_bg))
                .hover(move |style| style.bg(palette.hover_bg))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.state.settings_section = section;
                    cx.notify();
                }))
                .child(label.to_string()),
        );
    }

    let content = match state.settings_section {
        SettingsSection::Appearance => appearance_section(state, strings, palette, cx),
        SettingsSection::Terminal => terminal_section(state, strings, palette, cx),
        SettingsSection::Data => data_section(state, strings, palette),
        SettingsSection::About => about_section(state, strings, palette),
    };

    div()
        .flex_1()
        .flex()
        .flex_row()
        .min_w(px(0.0))
        .bg(palette.bg)
        .child(nav)
        .child(
            div()
                .id("settings-content")
                .flex_1()
                .min_w(px(0.0))
                .h_full()
                .p(px(16.0))
                .overflow_y_scroll()
                .scrollbar_width(px(0.0))
                .child(content),
        )
}

fn section_title(title: &'static str, palette: Palette) -> Div {
    div()
        .text_size(px(20.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette.font_1)
        .mb(px(16.0))
        .child(title.to_string())
}

fn setting_row(label: &'static str, control: impl IntoElement, palette: Palette) -> Div {
    div()
        .h(px(40.0))
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(palette.border_subtle)
        .child(
            div()
                .text_size(px(12.8))
                .text_color(palette.font_2)
                .child(label.to_string()),
        )
        .child(control)
}

fn appearance_section(
    state: &AppState,
    strings: &Strings,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> Div {
    let mut cards = div().flex().gap(px(12.0)).mb(px(16.0));
    for (index, theme_name) in [
        ThemeName::Nord,
        ThemeName::NordLight,
        ThemeName::GithubDark,
        ThemeName::GithubLight,
    ]
    .into_iter()
    .enumerate()
    {
        let selected = state.config.settings.theme == theme_name;
        let preview = theme::palette(theme_name).bg;
        cards = cards.child(
            div()
                .id(ElementId::named_usize("settings-theme", index))
                .w(px(104.0))
                .p(px(8.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.0))
                .bg(if selected {
                    palette.selected_bg
                } else {
                    palette.panel_bg
                })
                .border_1()
                .border_color(if selected {
                    palette.accent
                } else {
                    palette.border
                })
                .cursor_pointer()
                .hover(move |style| style.border_color(palette.accent))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.state.config.settings.theme = theme_name;
                    let next = theme::palette(theme_name);
                    theme::apply_to_component(cx, next, this.state.config.settings.terminal_font_size);
                    if let Err(error) = this.state.persist() {
                        this.state.notice = Some(error);
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .w_full()
                        .h(px(40.0))
                        .bg(preview)
                        .border_1()
                        .border_color(palette.border),
                )
                .child(
                    div()
                        .text_size(px(12.8))
                        .text_color(palette.font_1)
                        .child(theme::theme_label(theme_name, *strings).to_string()),
                ),
        );
    }

    let mut language_row = div().flex().gap(px(8.0));
    for (lang, label) in [
        (managi_core::Language::Zh, "中文"),
        (managi_core::Language::En, "English"),
    ] {
        let selected = state.config.settings.language == lang;
        language_row = language_row.child(
            div()
                .id(format!("settings-language-{label}"))
                .h(px(28.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(if selected {
                    palette.accent
                } else {
                    palette.border
                })
                .text_color(if selected {
                    palette.accent
                } else {
                    palette.font_1
                })
                .text_size(px(12.8))
                .cursor_pointer()
                .hover(move |style| style.border_color(palette.accent))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.state.config.settings.language = lang;
                    if let Err(error) = this.state.persist() {
                        this.state.notice = Some(error);
                    }
                    cx.notify();
                }))
                .child(label),
        );
    }

    div()
        .flex()
        .flex_col()
        .child(section_title(strings.appearance, palette))
        .child(
            div()
                .text_size(px(12.8))
                .text_color(palette.font_3)
                .mb(px(8.0))
                .child(strings.theme.to_string()),
        )
        .child(cards)
        .child(setting_row(strings.language, language_row, palette))
}

fn terminal_section(
    state: &AppState,
    strings: &Strings,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> Div {
    let size = state.config.settings.terminal_font_size;

    let stepper = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(step_button("-", palette).id("settings-font-minus").on_click(cx.listener(|this, _, _, cx| {
            let next = this
                .state
                .config
                .settings
                .terminal_font_size
                .saturating_sub(1)
                .max(MIN_TERMINAL_FONT_SIZE);
            this.state.config.settings.terminal_font_size = next;
            theme::apply_to_component(cx, theme::palette(this.state.config.settings.theme), next);
            if let Err(error) = this.state.persist() {
                this.state.notice = Some(error);
            }
            cx.notify();
        })))
        .child(
            div()
                .w(px(40.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .bg(palette.input_bg)
                .border_1()
                .border_color(palette.border)
                .text_size(px(12.8))
                .text_color(palette.font_1)
                .child(size.to_string()),
        )
        .child(step_button("+", palette).id("settings-font-plus").on_click(cx.listener(|this, _, _, cx| {
            let next = this
                .state
                .config
                .settings
                .terminal_font_size
                .saturating_add(1)
                .min(MAX_TERMINAL_FONT_SIZE);
            this.state.config.settings.terminal_font_size = next;
            theme::apply_to_component(cx, theme::palette(this.state.config.settings.theme), next);
            if let Err(error) = this.state.persist() {
                this.state.notice = Some(error);
            }
            cx.notify();
        })));

    let system_family = if cfg!(target_os = "windows") {
        "Consolas"
    } else if cfg!(target_os = "macos") {
        "Menlo"
    } else {
        "DejaVu Sans Mono"
    };

    let mut families = div().flex().gap(px(8.0));
    for (family, label) in [
        ("JetBrains Mono", "JetBrains Mono"),
        (system_family, strings.font_system),
    ] {
        let selected = state.config.settings.terminal_font_family == family;
        families = families.child(
            div()
                .id(format!("settings-family-{family}"))
                .h(px(28.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(if selected {
                    palette.accent
                } else {
                    palette.border
                })
                .text_color(if selected {
                    palette.accent
                } else {
                    palette.font_1
                })
                .text_size(px(12.8))
                .cursor_pointer()
                .hover(move |style| style.border_color(palette.accent))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.state.config.settings.terminal_font_family = family.to_string();
                    if let Err(error) = this.state.persist() {
                        this.state.notice = Some(error);
                    }
                    cx.notify();
                }))
                .child(label.to_string()),
        );
    }

    div()
        .flex()
        .flex_col()
        .child(section_title(strings.terminal, palette))
        .child(setting_row(strings.font_size, stepper, palette))
        .child(setting_row(strings.font_family, families, palette))
}

fn data_section(state: &AppState, strings: &Strings, palette: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .child(section_title(strings.data, palette))
        .child(
            div()
                .max_w(px(480.0))
                .text_size(px(12.8))
                .text_color(palette.font_3)
                .mb(px(12.0))
                .child(strings.data_desc.to_string()),
        )
        .child(
            div()
                .flex()
                .gap(px(8.0))
                .child(disabled_button(strings.export_config, palette))
                .child(disabled_button(strings.import_config, palette))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .text_size(px(11.2))
                        .text_color(palette.font_3)
                        .child(format!("{}: {}", state.version, state.config_path.display())),
                ),
        )
}

fn about_section(state: &AppState, strings: &Strings, palette: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(section_title(strings.about, palette))
        .child(
            div()
                .text_size(px(12.8))
                .text_color(palette.font_2)
                .child(format!("managi-gpui v{}", state.version)),
        )
        .child(
            div()
                .text_size(px(12.8))
                .text_color(palette.font_3)
                .child(strings.about_desc.to_string()),
        )
}

fn step_button(label: &'static str, palette: Palette) -> Div {
    div()
        .w(px(28.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(palette.border)
        .text_color(palette.font_1)
        .text_size(px(14.4))
        .cursor_pointer()
        .hover(move |style| style.border_color(palette.accent).text_color(palette.accent))
        .child(label.to_string())
}

fn disabled_button(label: &'static str, palette: Palette) -> Div {
    div()
        .h(px(30.0))
        .px(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(palette.border)
        .text_color(palette.font_1)
        .text_size(px(12.8))
        .opacity(0.5)
        .child(label.to_string())
}
