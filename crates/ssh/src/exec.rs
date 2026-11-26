//! 批量执行：每台主机一条非交互 exec 通道，输出整段回传。
//!
//! 命令不做 shell 转义，原样交给远端登录 shell。

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use russh::ChannelMsg;
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::session::{ConnectRequest, connect_and_authenticate};

const MAX_OUTPUT: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecEvent {
    Done { code: Option<u32>, output: String },
    Error(String),
}

#[derive(Debug)]
enum Command {
    Close,
}

pub struct ExecHandle {
    commands: UnboundedSender<Command>,
    events: Receiver<ExecEvent>,
    finished: bool,
}

impl ExecHandle {
    pub fn close(&self) {
        let _ = self.commands.send(Command::Close);
    }

    pub fn drain(&self) -> Vec<ExecEvent> {
        let mut events = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        events
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn mark_finished(&mut self) {
        self.finished = true;
    }
}

impl Drop for ExecHandle {
    fn drop(&mut self) {
        self.close();
    }
}

pub fn spawn(request: ConnectRequest, command: String) -> Result<ExecHandle, ExecError> {
    let (commands, command_rx) = unbounded_channel();
    let (event_tx, events) = mpsc::channel();

    std::thread::Builder::new()
        .name("managi-exec".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let result = match runtime {
                Ok(runtime) => runtime.block_on(run(request, command, command_rx, event_tx.clone())),
                Err(error) => Err(format!("启动批量线程失败: {error}")),
            };
            if let Err(error) = result {
                let _ = event_tx.send(ExecEvent::Error(error));
            }
        })
        .map_err(ExecError::Spawn)?;

    Ok(ExecHandle {
        commands,
        events,
        finished: false,
    })
}

async fn run(
    request: ConnectRequest,
    command: String,
    mut command_rx: UnboundedReceiver<Command>,
    events: mpsc::Sender<ExecEvent>,
) -> Result<(), String> {
    let mut session = connect_and_authenticate(&request).await?;
    let channel = session
        .channel_open_session()
        .await
        .map_err(|error| format!("打开会话失败: {error}"))?;
    channel
        .exec(true, command.as_bytes())
        .await
        .map_err(|error| format!("启动命令失败: {error}"))?;

    let mut output = Vec::new();
    let mut exit = None;
    loop {
        tokio::select! {
            command = command_rx.recv() => {
                if command.is_none() || matches!(command, Some(Command::Close)) {
                    break;
                }
            }
            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) => {
                        if output.len() < MAX_OUTPUT {
                            output.extend_from_slice(&data);
                        }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        if output.len() < MAX_OUTPUT {
                            output.extend_from_slice(&data);
                        }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        exit = Some(exit_status);
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
        }
    }

    if output.len() >= MAX_OUTPUT {
        output.truncate(MAX_OUTPUT);
        output.extend_from_slice("\n…输出已截断（上限 256 KB）".as_bytes());
    }
    let _ = events.send(ExecEvent::Done {
        code: exit,
        output: String::from_utf8_lossy(&output).into_owned(),
    });
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "", "managi")
        .await;
    // 让 UI 有稳定的一拍收尾，避免最后一条事件与线程退出竞争。
    tokio::time::sleep(Duration::from_millis(10)).await;
    Ok(())
}

#[derive(Debug, Error)]
pub enum ExecError {
    #[error("启动批量线程失败: {0}")]
    Spawn(#[from] std::io::Error),
}
