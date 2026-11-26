//! SSH / SFTP 适配层。
//!
//! UI 只应通过 `managi-core` 里的会话接口与本层交互，不直接依赖 russh 类型。

pub mod exec;
pub mod session;
pub mod sftp;
pub mod target;
pub mod trust;

pub use exec::{ExecError, ExecEvent, ExecHandle};
pub use session::{
    AuthMethod, ConnectRequest, SessionError, SessionEvent, SessionHandle,
};
pub use sftp::{
    SftpCommand, SftpEntry, SftpError, SftpEvent, SftpHandle, download_root, format_mtime,
    human_size,
};
pub use target::{SshTarget, TargetError};
pub use trust::{is_known_host, remember_host, TrustError};
