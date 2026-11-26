//! 一条终端标签的运行时：SSH 事件轮询 + alacritty 网格 + 尺寸。

use managi_ssh::{SessionEvent, SessionHandle};
use managi_term::{TerminalModel, TerminalSize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TerminalStatus {
    Connecting,
    Connected,
    Closed(String),
    Error(String),
}

pub(crate) struct TerminalRuntime {
    pub handle: SessionHandle,
    pub model: TerminalModel,
    pub status: TerminalStatus,
    pub columns: u16,
    pub rows: u16,
}

impl TerminalRuntime {
    pub(crate) fn new(handle: SessionHandle, columns: u16, rows: u16) -> Self {
        Self {
            handle,
            model: TerminalModel::new(
                TerminalSize::new(usize::from(columns), usize::from(rows)),
                10_000,
            ),
            status: TerminalStatus::Connecting,
            columns,
            rows,
        }
    }

    /// 排空事件通道；返回是否有新内容需要重绘。
    pub(crate) fn poll(&mut self) -> bool {
        let mut changed = false;
        for event in self.handle.drain() {
            match event {
                SessionEvent::Connected => {
                    self.status = TerminalStatus::Connected;
                    changed = true;
                }
                SessionEvent::Data(bytes) => {
                    self.model.advance(&bytes);
                    changed = true;
                }
                SessionEvent::Closed(reason) => {
                    if !matches!(self.status, TerminalStatus::Error(_)) {
                        self.status = TerminalStatus::Closed(reason);
                    }
                    self.handle.mark_disconnected();
                    changed = true;
                }
                SessionEvent::Error(error) => {
                    self.status = TerminalStatus::Error(error);
                    changed = true;
                }
            }
        }
        changed
    }

    pub(crate) fn resize(&mut self, columns: u16, rows: u16) {
        if self.columns == columns && self.rows == rows {
            return;
        }
        self.columns = columns;
        self.rows = rows;
        self.model
            .resize(TerminalSize::new(usize::from(columns), usize::from(rows)));
        self.handle.resize(columns, rows);
    }

    pub(crate) fn is_live(&self) -> bool {
        matches!(
            self.status,
            TerminalStatus::Connecting | TerminalStatus::Connected
        )
    }

    pub(crate) fn scroll(&mut self, lines: i32) {
        self.model.scroll(lines);
    }

    pub(crate) fn bracketed_paste(&self) -> bool {
        self.model.bracketed_paste()
    }
}
