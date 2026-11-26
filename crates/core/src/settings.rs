//! 界面与终端偏好。所有字段都有默认值，缺字段的旧配置也能读。

use serde::{Deserialize, Serialize};

pub const MIN_TERMINAL_FONT_SIZE: u16 = 8;
pub const MAX_TERMINAL_FONT_SIZE: u16 = 32;
pub const DEFAULT_TERMINAL_FONT_SIZE: u16 = 14;
pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = "JetBrains Mono";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeName {
    #[default]
    Nord,
    NordLight,
    GithubDark,
    GithubLight,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Zh,
    En,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeName,
    pub language: Language,
    pub terminal_font_size: u16,
    pub terminal_font_family: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeName::default(),
            language: Language::default(),
            terminal_font_size: DEFAULT_TERMINAL_FONT_SIZE,
            terminal_font_family: DEFAULT_TERMINAL_FONT_FAMILY.to_string(),
        }
    }
}

impl Settings {
    /// 把来自界面或旧配置的越界值拉回合法范围。
    pub fn normalize(&mut self) {
        self.terminal_font_size = self
            .terminal_font_size
            .clamp(MIN_TERMINAL_FONT_SIZE, MAX_TERMINAL_FONT_SIZE);

        let family = self.terminal_font_family.trim();
        if family.is_empty() {
            self.terminal_font_family = DEFAULT_TERMINAL_FONT_FAMILY.to_string();
        } else if family != self.terminal_font_family {
            self.terminal_font_family = family.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_stable() {
        let settings = Settings::default();
        assert_eq!(settings.theme, ThemeName::Nord);
        assert_eq!(settings.language, Language::Zh);
        assert_eq!(settings.terminal_font_size, 14);
    }

    #[test]
    fn normalize_clamps_and_repairs() {
        let mut settings = Settings {
            terminal_font_size: 100,
            terminal_font_family: "   ".into(),
            ..Settings::default()
        };
        settings.normalize();
        assert_eq!(settings.terminal_font_size, MAX_TERMINAL_FONT_SIZE);
        assert_eq!(settings.terminal_font_family, DEFAULT_TERMINAL_FONT_FAMILY);
    }
}
