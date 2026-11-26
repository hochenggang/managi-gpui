//! 一条 SSH 交互会话：std 线程 + 单线程 tokio + russh PTY。
//!
//! UI 只拿到 [`SessionHandle`]：写入/改尺寸/关闭 + 轮询事件。
//! 网络与解析都不在 GPUI 主线程上等待。

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, Handle};
use russh::keys::{PrivateKeyWithHashAlg, load_secret_key};
use russh::{ChannelMsg, Disconnect};
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::trust::{is_known_host, remember_host};

#[derive(Clone, Debug)]
pub enum AuthMethod {
    Password(String),
    PrivateKey {
        path: PathBuf,
        passphrase: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct ConnectRequest {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
    pub known_hosts_path: PathBuf,
    pub columns: u16,
    pub rows: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    Connected,
    Data(Vec<u8>),
    Closed(String),
    Error(String),
}

#[derive(Debug)]
enum Command {
    Write(Vec<u8>),
    Resize(u16, u16),
    Close,
}

/// UI 持有的会话句柄。事件通道是非阻塞的，由界面每帧 drain。
pub struct SessionHandle {
    commands: UnboundedSender<Command>,
    events: Receiver<SessionEvent>,
    disconnected: bool,
}

impl SessionHandle {
    pub fn write(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.commands.send(Command::Write(bytes));
        }
    }

    pub fn resize(&self, columns: u16, rows: u16) {
        let _ = self.commands.send(Command::Resize(columns, rows));
    }

    pub fn close(&self) {
        let _ = self.commands.send(Command::Close);
    }

    pub fn try_event(&self) -> Option<SessionEvent> {
        match self.events.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    pub fn drain(&self) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.try_event() {
            events.push(event);
        }
        events
    }

    pub fn is_disconnected(&self) -> bool {
        self.disconnected
    }

    pub fn mark_disconnected(&mut self) {
        self.disconnected = true;
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        self.close();
    }
}

/// 启动后台会话线程；连接过程在独立 tokio runtime 内完成。
pub fn spawn(request: ConnectRequest) -> Result<SessionHandle, SessionError> {
    let (commands, command_rx) = unbounded_channel();
    let (event_tx, events) = mpsc::channel();

    std::thread::Builder::new()
        .name("managi-ssh".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let result = match runtime {
                Ok(runtime) => runtime.block_on(run(request, command_rx, event_tx.clone())),
                Err(error) => Err(format!("启动网络线程失败: {error}")),
            };
            if let Err(error) = result {
                let _ = event_tx.send(SessionEvent::Error(error.clone()));
                let _ = event_tx.send(SessionEvent::Closed(error));
            }
        })
        .map_err(SessionError::Spawn)?;

    Ok(SessionHandle {
        commands,
        events,
        disconnected: false,
    })
}

pub(crate) struct ClientHandler {
    host: String,
    port: u16,
    known_hosts_path: PathBuf,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = server_public_key.public_key();
        match is_known_host(&self.host, self.port, &key, &self.known_hosts_path) {
            Ok(true) => Ok(true),
            Ok(false) => {
                let path = self.known_hosts_path.clone();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match remember_host(&self.host, self.port, &key, &path) {
                    Ok(()) => {
                        tracing::info!(host = %self.host, port = self.port, "首次记录主机密钥");
                        Ok(true)
                    }
                    Err(error) => {
                        tracing::error!(%error, "写入 known_hosts 失败");
                        Ok(false)
                    }
                }
            }
            Err(error) => {
                tracing::error!(%error, "读取 known_hosts 失败");
                Ok(false)
            }
        }
    }
}

async fn run(
    request: ConnectRequest,
    mut command_rx: UnboundedReceiver<Command>,
    events: mpsc::Sender<SessionEvent>,
) -> Result<(), String> {
    let connect = connect_and_authenticate(&request).await?;
    let mut session = connect;

    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|error| format!("打开会话失败: {error}"))?;

    channel
        .request_pty(
            false,
            "xterm-256color",
            u32::from(request.columns.max(1)),
            u32::from(request.rows.max(1)),
            0,
            0,
            &[],
        )
        .await
        .map_err(|error| format!("申请 PTY 失败: {error}"))?;

    channel
        .request_shell(true)
        .await
        .map_err(|error| format!("启动 shell 失败: {error}"))?;

    let _ = events.send(SessionEvent::Connected);
    tracing::info!(host = %request.host, port = request.port, "SSH 已连接");

    loop {
        tokio::select! {
            command = command_rx.recv() => {
                match command {
                    Some(Command::Write(bytes)) => {
                        if let Err(error) = channel.data_bytes(bytes).await {
                            tracing::error!(%error, "写入 SSH 失败");
                        }
                    }
                    Some(Command::Resize(columns, rows)) => {
                        if let Err(error) = channel
                            .window_change(u32::from(columns.max(1)), u32::from(rows.max(1)), 0, 0)
                            .await
                        {
                            tracing::error!(%error, "更新窗口尺寸失败");
                        }
                    }
                    Some(Command::Close) | None => break,
                }
            }
            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) => {
                        let _ = events.send(SessionEvent::Data(data.to_vec()));
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let _ = events.send(SessionEvent::Data(data.to_vec()));
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) => break,
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        let _ = events.send(SessionEvent::Closed(format!("进程退出（{exit_status}）")));
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
        }
    }

    let _ = session
        .disconnect(Disconnect::ByApplication, "", "managi")
        .await;
    let _ = events.send(SessionEvent::Closed("连接已关闭".into()));
    Ok(())
}

/// 建立连接并完成认证；终端、SFTP、批量执行共用同一套超时与 TOFU 策略。
pub(crate) async fn connect_and_authenticate(
    request: &ConnectRequest,
) -> Result<Handle<ClientHandler>, String> {
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(60)),
        nodelay: true,
        ..client::Config::default()
    });
    let handler = ClientHandler {
        host: request.host.clone(),
        port: request.port,
        known_hosts_path: request.known_hosts_path.clone(),
    };

    let connect = tokio::time::timeout(
        Duration::from_secs(20),
        client::connect(config, (request.host.as_str(), request.port), handler),
    )
    .await
    .map_err(|_| "连接超时（20 秒）".to_string())?
    .map_err(|error| format!("连接失败: {error}"))?;

    let mut session: Handle<ClientHandler> = connect;
    authenticate(&mut session, request).await?;
    Ok(session)
}

async fn authenticate(
    session: &mut Handle<ClientHandler>,
    request: &ConnectRequest,
) -> Result<(), String> {
    let result = match &request.auth {
        AuthMethod::Password(password) => {
            tokio::time::timeout(
                Duration::from_secs(25),
                session.authenticate_password(request.username.clone(), password.clone()),
            )
            .await
            .map_err(|_| "认证超时（25 秒）".to_string())?
            .map_err(|error| format!("认证失败: {error}"))?
        }
        AuthMethod::PrivateKey { path, passphrase } => {
            let key = load_secret_key(path, passphrase.as_deref())
                .map_err(|error| format!("读取私钥失败: {error}"))?;
            let hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(|error| format!("协商 RSA 算法失败: {error}"))?
                .flatten();
            tokio::time::timeout(
                Duration::from_secs(25),
                session.authenticate_publickey(
                    request.username.clone(),
                    PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                ),
            )
            .await
            .map_err(|_| "认证超时（25 秒）".to_string())?
            .map_err(|error| format!("认证失败: {error}"))?
        }
    };

    if result.success() {
        Ok(())
    } else {
        Err("用户名、密码或私钥不正确".into())
    }
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("启动会话线程失败: {0}")]
    Spawn(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_channel_is_non_blocking() {
        let (tx, rx) = mpsc::channel();
        tx.send(SessionEvent::Connected).unwrap();
        let handle = SessionHandle {
            commands: unbounded_channel().0,
            events: rx,
            disconnected: false,
        };
        assert_eq!(handle.try_event(), Some(SessionEvent::Connected));
        assert_eq!(handle.try_event(), None);
    }
}
