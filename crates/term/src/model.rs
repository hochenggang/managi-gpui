//! 包装 `alacritty_terminal`：吃字节流、维护网格、导出可见文本。
//!
//! 不启动本地 PTY：SSH 通道的字节直接喂给 [`TerminalModel::advance`]。
//! 这样解析逻辑可以在没有网络、没有窗口的 CI 里单测。

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
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
    search: Option<SearchState>,
}

#[derive(Clone, Copy, Debug)]
struct SearchMatch {
    line: i32,
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
struct SearchState {
    query: String,
    matches: Vec<SearchMatch>,
    current: usize,
}

/// 一帧终端画面。UI 只读取这里的数据，不直接碰 alacritty 类型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSnapshot {
    pub rows: Vec<SnapshotRow>,
    pub cursor: SnapshotCursor,
    pub mode: TermMode,
    pub display_offset: usize,
    /// 每行选中的可见列区间 `[start, end)`，下标是屏幕行号。
    pub selection_rows: Vec<Option<(usize, usize)>>,
    /// 搜索命中；`current` 为当前定位项。
    pub search_spans: Vec<SnapshotSpan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotSpan {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    pub current: bool,
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
            search: None,
        }
    }

    /// 喂入原始字节（SSH 通道输出或本地测试数据）。
    pub fn advance(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        if self.search.as_ref().is_some_and(|search| !search.query.is_empty()) {
            self.recompute_search();
        }
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

    /// 鼠标按下时开始选区。行列都是可见屏幕坐标。
    pub fn begin_selection(&mut self, line: usize, column: usize, kind: SelectionType) {
        let offset = self.term.grid().display_offset();
        let point = alacritty_terminal::term::viewport_to_point(
            offset,
            Point::new(line, Column(column.min(self.size.columns.saturating_sub(1)))),
        );
        self.term.selection = Some(Selection::new(kind, point, Side::Left));
    }

    /// 按点击次数选择语义：1 次精确、2 次按词、3 次整行。
    pub fn begin_selection_at(&mut self, line: usize, column: usize, clicks: usize) {
        let kind = match clicks {
            2 => SelectionType::Semantic,
            3 => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        self.begin_selection(line, column, kind);
    }

    /// 鼠标拖动时更新选区末端；`tail` 表示落在字符格右半边，应包含该字符。
    pub fn update_selection(&mut self, line: usize, column: usize, tail: bool) {
        let offset = self.term.grid().display_offset();
        let point = alacritty_terminal::term::viewport_to_point(
            offset,
            Point::new(line, Column(column.min(self.size.columns.saturating_sub(1)))),
        );
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, if tail { Side::Right } else { Side::Left });
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    pub fn has_selection(&self) -> bool {
        self.term.selection.as_ref().is_some_and(|selection| !selection.is_empty())
    }

    pub fn selection_text(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    /// 全选（含回看历史）。
    pub fn select_all(&mut self) {
        let grid = self.term.grid();
        let top = grid.topmost_line();
        let bottom = grid.bottommost_line();
        let last_column = Column(self.size.columns.saturating_sub(1));
        let mut selection = Selection::new(SelectionType::Simple, Point::new(top, Column(0)), Side::Left);
        selection.update(Point::new(bottom, last_column), Side::Left);
        self.term.selection = Some(selection);
    }

    /// Ctrl+F 搜索；空查询等同于清除搜索。
    pub fn set_search(&mut self, query: &str) {
        let query = query.trim().to_string();
        if query.is_empty() {
            self.search = None;
            return;
        }
        let current = self.search.as_ref().map_or(0, |search| search.current);
        self.search = Some(SearchState {
            query,
            matches: Vec::new(),
            current,
        });
        self.recompute_search();
        if let Some((line, start)) = self
            .search
            .as_ref()
            .and_then(|search| search.matches.get(search.current))
            .map(|item| (item.line, item.start))
        {
            self.term
                .scroll_to_point(Point::new(Line(line), Column(start)));
        }
    }

    pub fn clear_search(&mut self) {
        self.search = None;
    }

    /// 返回 `(当前序号, 总数)`，序号从 1 开始；没有查询时返回 `None`。
    pub fn search_status(&self) -> Option<(usize, usize)> {
        let search = self.search.as_ref()?;
        let total = search.matches.len();
        if total == 0 {
            Some((0, 0))
        } else {
            Some((search.current.min(total - 1) + 1, total))
        }
    }

    /// 跳到下一个/上一个命中；返回是否真的移动了。
    pub fn search_next(&mut self, forward: bool) -> bool {
        let Some(search) = self.search.as_mut() else {
            return false;
        };
        if search.matches.is_empty() {
            return false;
        }
        let total = search.matches.len();
        search.current = if forward {
            (search.current + 1) % total
        } else {
            (search.current + total - 1) % total
        };
        let target = search.matches[search.current];
        self.term
            .scroll_to_point(Point::new(Line(target.line), Column(target.start)));
        true
    }

    /// 当前屏幕行、列所在的可点击链接（只认 http/https）。
    pub fn url_at(&self, screen_line: usize, column: usize) -> Option<String> {
        if screen_line >= self.size.screen_lines {
            return None;
        }
        let offset = self.term.grid().display_offset() as i32;
        let line = &self.term.grid()[Line(screen_line as i32 - offset)];
        let mut text = String::with_capacity(self.size.columns);
        let mut starts: Vec<Option<usize>> = vec![None; self.size.columns];
        for col in 0..self.size.columns {
            let cell = &line[Column(col)];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            starts[col] = Some(text.len());
            text.push(cell.c);
        }
        let byte_at = |col: usize| -> usize {
            starts
                .get(col)
                .and_then(|start| *start)
                .or_else(|| {
                    starts
                        .get(col..)
                        .and_then(|tail| tail.iter().find_map(|start| *start))
                })
                .unwrap_or(text.len())
        };
        let cursor = byte_at(column.min(self.size.columns.saturating_sub(1)));
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let mut start_index = chars
            .iter()
            .position(|(byte, _)| *byte >= cursor)
            .unwrap_or(chars.len());
        while start_index > 0 && !chars[start_index - 1].1.is_whitespace() {
            start_index -= 1;
        }
        let mut end_index = start_index;
        while end_index < chars.len() && !chars[end_index].1.is_whitespace() {
            end_index += 1;
        }
        let start = chars.get(start_index).map_or(text.len(), |(byte, _)| *byte);
        let end = chars.get(end_index).map_or(text.len(), |(byte, _)| *byte);
        let token = text[start..end]
            .trim_matches(|c: char| matches!(c, ')' | ']' | '}' | ',' | ';' | '"' | '\''));
        if token.starts_with("http://") || token.starts_with("https://") {
            Some(token.to_string())
        } else {
            None
        }
    }

    fn recompute_search(&mut self) {
        let Some(search) = self.search.as_ref() else {
            return;
        };
        let query: Vec<char> = search
            .query
            .chars()
            .map(|c| c.to_lowercase().next().unwrap_or(c))
            .collect();
        if query.is_empty() {
            return;
        }
        let grid = self.term.grid();
        let top = grid.topmost_line().0;
        let bottom = grid.bottommost_line().0;
        let mut matches = Vec::new();
        for line_index in top..=bottom {
            let row = &grid[Line(line_index)];
            let mut chars: Vec<(usize, char)> = Vec::with_capacity(self.size.columns);
            for col in 0..self.size.columns {
                let cell = &row[Column(col)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                chars.push((col, cell.c.to_lowercase().next().unwrap_or(cell.c)));
            }
            if chars.is_empty() || query.len() > chars.len() {
                continue;
            }
            for start in 0..=chars.len() - query.len() {
                if query
                    .iter()
                    .zip(chars[start..start + query.len()].iter())
                    .all(|(needle, (_, hay))| *needle == *hay)
                {
                    matches.push(SearchMatch {
                        line: line_index,
                        start: chars[start].0,
                        end: chars[start + query.len() - 1].0 + 1,
                    });
                }
            }
        }
        if let Some(search) = self.search.as_mut() {
            search.matches = matches;
            search.current = if search.matches.is_empty() {
                0
            } else {
                search.current.min(search.matches.len() - 1)
            };
        }
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
        let selection_rows = self.selection_rows(offset);
        let search_spans = self.search_spans(offset);
        TerminalSnapshot {
            rows,
            cursor: SnapshotCursor {
                line: cursor_line.max(0) as usize,
                column: content.cursor.point.column.0,
                visible: content.mode.contains(TermMode::SHOW_CURSOR),
            },
            mode: content.mode,
            display_offset: content.display_offset,
            selection_rows,
            search_spans,
        }
    }

    fn selection_rows(&self, offset: i32) -> Vec<Option<(usize, usize)>> {
        let mut rows = vec![None; self.size.screen_lines];
        let Some(range) = self
            .term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&self.term))
        else {
            return rows;
        };
        for (row, slot) in rows.iter_mut().enumerate() {
            let line = row as i32 - offset;
            if line < range.start.line.0 || line > range.end.line.0 {
                continue;
            }
            let (start, end) = if range.is_block || range.start.line == range.end.line {
                (range.start.column.0, range.end.column.0 + 1)
            } else if line == range.start.line.0 {
                (range.start.column.0, self.size.columns)
            } else if line == range.end.line.0 {
                (0, range.end.column.0 + 1)
            } else {
                (0, self.size.columns)
            };
            if start < end {
                *slot = Some((start.min(self.size.columns), end.min(self.size.columns)));
            }
        }
        rows
    }

    fn search_spans(&self, offset: i32) -> Vec<SnapshotSpan> {
        let Some(search) = self.search.as_ref() else {
            return Vec::new();
        };
        search
            .matches
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let row = item.line + offset;
                if row < 0 || row as usize >= self.size.screen_lines {
                    return None;
                }
                Some(SnapshotSpan {
                    row: row as usize,
                    start: item.start.min(self.size.columns),
                    end: item.end.min(self.size.columns),
                    current: index == search.current,
                })
            })
            .collect()
    }

    /// 滚轮回看：正数向上翻历史，负数回到当前。
    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }

    /// 可见区的纯文本快照，用于自检/复制/调试，不用于绘制。
    pub fn screen_text(&self) -> String {
        let offset = self.term.grid().display_offset() as i32;
        let mut out = String::with_capacity(self.size.screen_lines * (self.size.columns + 1));
        for line in 0..self.size.screen_lines {
            let row = &self.term.grid()[Line(line as i32 - offset)];
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

    #[test]
    fn supports_selection_and_search() {
        let mut model = TerminalModel::new(TerminalSize::new(40, 4), 100);
        model.advance(b"hello world\r\nsecond line");
        model.begin_selection_at(0, 0, 1);
        model.update_selection(0, 4, true);
        assert_eq!(model.selection_text().as_deref(), Some("hello"));
        assert!(model.snapshot().selection_rows[0].is_some());

        model.set_search("line");
        let (current, total) = model.search_status().unwrap();
        assert_eq!((current, total), (1, 1));
        assert_eq!(model.snapshot().search_spans.len(), 1);
        model.clear_search();
        assert!(model.search_status().is_none());
    }
}
