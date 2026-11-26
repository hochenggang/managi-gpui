//! 包装 `alacritty_terminal`：吃字节流、维护网格、导出可见文本。
//!
//! 不启动本地 PTY：SSH 通道的字节直接喂给 [`TerminalModel::advance`]。
//! 这样解析逻辑可以在没有网络、没有窗口的 CI 里单测。

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, Processor};

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

/// 一帧终端画面。UI 只读取这里的数据，不直接碰 alacritty 类型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSnapshot {
    pub rows: Vec<SnapshotRow>,
    pub cursor: SnapshotCursor,
    pub mode: TermMode,
    pub display_offset: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotRow {
    /// 同一行里颜色/样式相同的连续字符。
    pub runs: Vec<SnapshotRun>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotRun {
    pub text: String,
    pub fg: SnapshotColor,
    pub bg: SnapshotColor,
    pub bold: bool,
    pub underline: bool,
}

/// 终端颜色：具体 RGB，或调色板索引（0–15 ANSI、256 前景、257 背景）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotColor {
    Rgb(u8, u8, u8),
    Index(u16),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotCursor {
    pub line: usize,
    pub column: usize,
    pub visible: bool,
}

impl SnapshotColor {
    fn from_alacritty(color: Color) -> Self {
        match color {
            Color::Spec(rgb) => Self::Rgb(rgb.r, rgb.g, rgb.b),
            Color::Indexed(index) => Self::Index(u16::from(index)),
            Color::Named(named) => Self::Index(named as u16),
        }
    }
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

    pub fn bracketed_paste(&self) -> bool {
        self.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// 复制可见区，供 GPUI 绘制。
    pub fn snapshot(&self) -> TerminalSnapshot {
        let offset = self.term.grid().display_offset() as i32;
        let mut rows = Vec::with_capacity(self.size.screen_lines);

        for row in 0..self.size.screen_lines {
            let line = &self.term.grid()[Line(row as i32 - offset)];
            let mut runs: Vec<SnapshotRun> = Vec::new();

            for column in 0..self.size.columns {
                let cell = &line[Column(column)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }

                let mut fg = SnapshotColor::from_alacritty(cell.fg);
                let mut bg = SnapshotColor::from_alacritty(cell.bg);
                if cell.flags.contains(Flags::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let bold = cell.flags.contains(Flags::BOLD);
                let underline = cell.flags.intersects(Flags::ALL_UNDERLINES);

                match runs.last_mut() {
                    Some(run)
                        if run.fg == fg
                            && run.bg == bg
                            && run.bold == bold
                            && run.underline == underline =>
                    {
                        run.text.push(cell.c);
                    }
                    _ => runs.push(SnapshotRun {
                        text: cell.c.to_string(),
                        fg,
                        bg,
                        bold,
                        underline,
                    }),
                }
            }

            rows.push(SnapshotRow { runs });
        }

        let content = self.term.renderable_content();
        let cursor_line = content.cursor.point.line.0 - content.display_offset as i32;
        TerminalSnapshot {
            rows,
            cursor: SnapshotCursor {
                line: cursor_line.max(0) as usize,
                column: content.cursor.point.column.0,
                visible: content.mode.contains(TermMode::SHOW_CURSOR),
            },
            mode: content.mode,
            display_offset: content.display_offset,
        }
    }

    /// 滚轮回看：正数向上翻历史，负数回到当前。
    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
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

    #[test]
    fn snapshot_keeps_runs_and_cursor() {
        let mut model = TerminalModel::new(TerminalSize::new(20, 4), 100);
        model.advance(b"plain\x1b[31mred\x1b[0m");
        let snapshot = model.snapshot();
        let first = &snapshot.rows[0];
        assert!(first.runs.len() >= 2);
        assert_eq!(first.runs[0].text, "plain");
        assert!(first.runs[1].text.starts_with("red"));
        assert_eq!(snapshot.cursor.column, 8);
    }
}
