//! 包装 `alacritty_terminal`：吃字节流、维护网格、导出可见文本。
//!
//! 不启动本地 PTY：SSH 通道的字节直接喂给 [`TerminalModel::advance`]。
//! 这样解析逻辑可以在没有网络、没有窗口的 CI 里单测。

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;

/// 网格尺寸（单位是字符格子，不是像素）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: usize,
    pub screen_lines: usize,
}

impl TerminalSize {
    pub fn new(columns: usize, screen_lines: usize) -> Self {
        Self {
            columns: columns.max(1),
            screen_lines: screen_lines.max(1),
        }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

pub struct TerminalModel {
    term: Term<VoidListener>,
    parser: Processor,
    size: TerminalSize,
}

impl TerminalModel {
    /// `scrollback` 是回看行数上限；0 表示不回看。
    pub fn new(size: TerminalSize, scrollback: usize) -> Self {
        let config = Config {
            scrolling_history: scrollback,
            ..Config::default()
        };
        let term = Term::new(config, &size, VoidListener);
        Self {
            term,
            parser: Processor::new(),
            size,
        }
    }

    /// 喂入原始字节（SSH 通道输出或本地测试数据）。
    pub fn advance(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    /// 窗口尺寸变化时调用；与当前尺寸相同则直接返回。
    pub fn resize(&mut self, size: TerminalSize) {
        if size == self.size {
            return;
        }
        self.term.resize(size);
        self.size = size;
    }

    pub fn size(&self) -> TerminalSize {
        self.size
    }

    /// 当前终端模式（应用光标、括号粘贴、鼠标上报等），供按键编码使用。
    pub fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    /// 可见区的纯文本快照，用于自检/复制/调试，不用于绘制。
    pub fn screen_text(&self) -> String {
        let mut out = String::with_capacity(self.size.screen_lines * (self.size.columns + 1));
        for line in 0..self.size.screen_lines {
            let row = &self.term.grid()[Line(line as i32)];
            let mut text = String::with_capacity(self.size.columns);
            for column in 0..self.size.columns {
                text.push(row[Column(column)].c);
            }
            out.push_str(text.trim_end());
            out.push('\n');
        }
        out.truncate(out.trim_end().len());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_text_and_sgr() {
        let mut model = TerminalModel::new(TerminalSize::new(40, 6), 100);
        model.advance(b"hello\x1b[31m red\x1b[0m");
        assert_eq!(model.screen_text().lines().next(), Some("hello red"));
    }

    #[test]
    fn newline_moves_to_next_row() {
        let mut model = TerminalModel::new(TerminalSize::new(20, 4), 100);
        model.advance(b"abc\r\ndef");
        let text = model.screen_text();
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("abc"));
        assert_eq!(lines.next(), Some("def"));
    }

    #[test]
    fn clear_screen_empties_visible_area() {
        let mut model = TerminalModel::new(TerminalSize::new(20, 4), 100);
        model.advance(b"abc\x1b[2J");
        assert_eq!(model.screen_text(), "");
    }

    #[test]
    fn resize_updates_reporting_size() {
        let mut model = TerminalModel::new(TerminalSize::new(80, 24), 100);
        model.resize(TerminalSize::new(120, 30));
        assert_eq!(model.size(), TerminalSize::new(120, 30));
    }
}
