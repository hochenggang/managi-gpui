//! 终端网格。渲染与输入在 `managi-ui`，这里只做状态。

pub mod keys;
pub mod model;

pub use keys::encode_key;
pub use model::{
    SnapshotColor, SnapshotCursor, SnapshotRow, SnapshotRun, SnapshotSpan, TerminalModel,
    TerminalSize, TerminalSnapshot,
};
