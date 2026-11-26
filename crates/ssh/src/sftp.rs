//! SFTP 文件会话：与终端同一套连接/认证，独占一条 subsystem 通道。
//!
//! 上传/下载在后台线程内分块读写，UI 只读事件、只发命令。

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::session::{ConnectRequest, connect_and_authenticate};

const CHUNK: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SftpEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SftpEvent {
    Ready { home: String },
    Entries { path: String, items: Vec<SftpEntry> },
    Progress { label: String, percent: u8 },
    Done { label: String },
    Failed { label: String, message: String },
    Closed(String),
}

#[derive(Clone, Debug)]
pub enum SftpCommand {
    List(String),
    Download { remote: String, local: PathBuf },
    Upload { local: PathBuf, remote: String },
    Delete { path: String, is_dir: bool },
    Mkdir(String),
    Close,
}

pub struct SftpHandle {
    commands: UnboundedSender<SftpCommand>,
    events: Receiver<SftpEvent>,
    closed: bool,
}

impl SftpHandle {
    pub fn send(&self, command: SftpCommand) {
        let _ = self.commands.send(command);
    }

    pub fn close(&self) {
        let _ = self.commands.send(SftpCommand::Close);
    }

    pub fn drain(&self) -> Vec<SftpEvent> {
        let mut events = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        events
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn mark_closed(&mut self) {
        self.closed = true;
    }
}

impl Drop for SftpHandle {
    fn drop(&mut self) {
        self.close();
    }
}

pub fn spawn(request: ConnectRequest) -> Result<SftpHandle, SftpError> {
    let (commands, command_rx) = unbounded_channel();
    let (event_tx, events) = mpsc::channel();

    std::thread::Builder::new()
        .name("managi-sftp".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let result = match runtime {
                Ok(runtime) => runtime.block_on(run(request, command_rx, event_tx.clone())),
                Err(error) => Err(format!("启动 SFTP 线程失败: {error}")),
            };
            if let Err(error) = result {
                let _ = event_tx.send(SftpEvent::Failed {
                    label: "连接".into(),
                    message: error.clone(),
                });
                let _ = event_tx.send(SftpEvent::Closed(error));
            }
        })
        .map_err(SftpError::Spawn)?;

    Ok(SftpHandle {
        commands,
        events,
        closed: false,
    })
}

async fn run(
    request: ConnectRequest,
    mut command_rx: UnboundedReceiver<SftpCommand>,
    events: mpsc::Sender<SftpEvent>,
) -> Result<(), String> {
    let mut session = connect_and_authenticate(&request).await?;
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|error| format!("打开 SFTP 通道失败: {error}"))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|error| format!("启动 SFTP 子系统失败: {error}"))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|error| format!("SFTP 握手失败: {error}"))?;
    sftp.set_timeout(20);

    let home = sftp
        .canonicalize(".")
        .await
        .map(|path| path)
        .unwrap_or_else(|_| "/".to_string());
    let _ = events.send(SftpEvent::Ready { home });

    loop {
        let Some(command) = command_rx.recv().await else {
            break;
        };
        match command {
            SftpCommand::List(path) => {
                let result = list_dir(&sftp, &path).await;
                match result {
                    Ok(items) => {
                        let _ = events.send(SftpEvent::Entries { path, items });
                    }
                    Err(error) => {
                        let _ = events.send(SftpEvent::Failed {
                            label: path,
                            message: error,
                        });
                    }
                }
            }
            SftpCommand::Download { remote, local } => {
                let label = file_label(&remote);
                let result = download(&sftp, &remote, &local, &events, &label).await;
                match result {
                    Ok(()) => {
                        let _ = events.send(SftpEvent::Done {
                            label: format!("{label} → {}", local.display()),
                        });
                    }
                    Err(error) => {
                        let _ = events.send(SftpEvent::Failed {
                            label,
                            message: error,
                        });
                    }
                }
            }
            SftpCommand::Upload { local, remote } => {
                let label = format!(
                    "{} → {}",
                    local
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| local.display().to_string()),
                    remote
                );
                let result = upload(&sftp, &local, &remote, &events, &label).await;
                match result {
                    Ok(()) => {
                        let _ = events.send(SftpEvent::Done { label });
                    }
                    Err(error) => {
                        let _ = events.send(SftpEvent::Failed {
                            label,
                            message: error,
                        });
                    }
                }
            }
            SftpCommand::Delete { path, is_dir } => {
                let label = file_label(&path);
                let result = if is_dir {
                    sftp.remove_dir(&path).await.map_err(|error| {
                        format!("删除目录失败（仅支持空目录）: {error}")
                    })
                } else {
                    sftp.remove_file(&path)
                        .await
                        .map_err(|error| format!("删除文件失败: {error}"))
                };
                match result {
                    Ok(()) => {
                        let _ = events.send(SftpEvent::Done { label });
                    }
                    Err(error) => {
                        let _ = events.send(SftpEvent::Failed {
                            label,
                            message: error,
                        });
                    }
                }
            }
            SftpCommand::Mkdir(path) => {
                let label = file_label(&path);
                match sftp.create_dir(&path).await {
                    Ok(()) => {
                        let _ = events.send(SftpEvent::Done { label });
                    }
                    Err(error) => {
                        let _ = events.send(SftpEvent::Failed {
                            label,
                            message: format!("新建目录失败: {error}"),
                        });
                    }
                }
            }
            SftpCommand::Close => break,
        }
    }

    let _ = sftp.close().await;
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "", "managi")
        .await;
    let _ = events.send(SftpEvent::Closed("连接已关闭".into()));
    Ok(())
}

async fn list_dir(sftp: &SftpSession, path: &str) -> Result<Vec<SftpEntry>, String> {
    let mut items = Vec::new();
    let read_dir = sftp
        .read_dir(path)
        .await
        .map_err(|error| format!("读取目录失败: {error}"))?;
    for entry in read_dir {
        let metadata = entry.metadata();
        items.push(SftpEntry {
            name: entry.file_name(),
            is_dir: entry.file_type().is_dir(),
            size: metadata.size.unwrap_or(0),
            modified: metadata.mtime.unwrap_or(0),
        });
    }
    items.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(items)
}

async fn download(
    sftp: &SftpSession,
    remote: &str,
    local: &PathBuf,
    events: &mpsc::Sender<SftpEvent>,
    label: &str,
) -> Result<(), String> {
    let mut remote_file = sftp
        .open(remote)
        .await
        .map_err(|error| format!("打开远端文件失败: {error}"))?;
    if let Some(parent) = local.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建本地目录失败: {error}"))?;
    }
    let mut local_file = tokio::fs::File::create(local)
        .await
        .map_err(|error| format!("创建本地文件失败: {error}"))?;
    let total = remote_file
        .metadata()
        .await
        .ok()
        .and_then(|metadata| metadata.size)
        .unwrap_or(0);

    let mut buffer = vec![0u8; CHUNK];
    let mut done = 0u64;
    loop {
        let read = remote_file
            .read(&mut buffer)
            .await
            .map_err(|error| format!("读取远端文件失败: {error}"))?;
        if read == 0 {
            break;
        }
        local_file
            .write_all(&buffer[..read])
            .await
            .map_err(|error| format!("写入本地文件失败: {error}"))?;
        done += read as u64;
        let percent = if total > 0 {
            ((done.saturating_mul(100) / total).min(99)) as u8
        } else {
            0
        };
        let _ = events.send(SftpEvent::Progress {
            label: label.to_string(),
            percent,
        });
    }
    local_file
        .flush()
        .await
        .map_err(|error| format!("刷新本地文件失败: {error}"))?;
    Ok(())
}

async fn upload(
    sftp: &SftpSession,
    local: &PathBuf,
    remote: &str,
    events: &mpsc::Sender<SftpEvent>,
    label: &str,
) -> Result<(), String> {
    let metadata = std::fs::metadata(local)
        .map_err(|error| format!("读取本地文件失败: {error}"))?;
    if metadata.is_dir() {
        return Err("暂不支持上传目录".into());
    }
    let mut local_file = tokio::fs::File::open(local)
        .await
        .map_err(|error| format!("打开本地文件失败: {error}"))?;
    let mut remote_file = sftp
        .open_with_flags(
            remote,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(|error| format!("创建远端文件失败: {error}"))?;

    let total = metadata.len();
    let mut buffer = vec![0u8; CHUNK];
    let mut done = 0u64;
    loop {
        let read = local_file
            .read(&mut buffer)
            .await
            .map_err(|error| format!("读取本地文件失败: {error}"))?;
        if read == 0 {
            break;
        }
        remote_file
            .write_all(&buffer[..read])
            .await
            .map_err(|error| format!("写入远端文件失败: {error}"))?;
        done += read as u64;
        let percent = if total > 0 {
            ((done.saturating_mul(100) / total).min(99)) as u8
        } else {
            0
        };
        let _ = events.send(SftpEvent::Progress {
            label: label.to_string(),
            percent,
        });
    }
    remote_file
        .flush()
        .await
        .map_err(|error| format!("刷新远端文件失败: {error}"))?;
    Ok(())
}

fn file_label(path: &str) -> String {
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(path)
        .to_string()
}

#[derive(Debug, Error)]
pub enum SftpError {
    #[error("启动 SFTP 线程失败: {0}")]
    Spawn(#[from] std::io::Error),
}

/// 本地下载目录：`<Downloads>/managi`，没有 Downloads 时退回临时目录。
pub fn download_root() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    match home {
        Some(home) => {
            let downloads = home.join("Downloads");
            if downloads.exists() {
                downloads.join("managi")
            } else {
                home.join("managi-downloads")
            }
        }
        None => std::env::temp_dir().join("managi-downloads"),
    }
}

/// 给小文件一个固定的人读尺寸；避免引入格式化依赖。
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// mtime 秒 → `YYYY-MM-DD HH:MM`（UTC 近似，仅用于列表展示）。
pub fn format_mtime(seconds: u32) -> String {
    if seconds == 0 {
        return "-".into();
    }
    let days = i64::from(seconds) / 86_400;
    let secs = i64::from(seconds) % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { y + 1 } else { y }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_sizes() {
        assert_eq!(file_label("/var/log/syslog"), "syslog");
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(format_mtime(0), "-");
        assert!(format_mtime(1_700_000_000).starts_with("2023-11-14"));
    }
}
