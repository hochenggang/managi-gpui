//! 批量执行界面运行时：每台主机一个 exec 句柄与结果卡片状态。

use managi_ssh::{ExecEvent, ExecHandle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostState {
    Running,
    Done { code: Option<u32>, output: String },
    Error(String),
    Stopped,
}

impl HostState {
    pub fn finished(&self) -> bool {
        !matches!(self, HostState::Running)
    }
}

pub struct HostRun {
    pub name: String,
    pub node_id: String,
    pub handle: Option<ExecHandle>,
    pub state: HostState,
}

pub struct BatchRuntime {
    pub command: String,
    pub hosts: Vec<HostRun>,
}

impl BatchRuntime {
    pub fn new(command: String) -> Self {
        Self {
            command,
            hosts: Vec::new(),
        }
    }

    pub fn push(&mut self, name: String, node_id: String, handle: Option<ExecHandle>) {
        let state = if handle.is_some() {
            HostState::Running
        } else {
            HostState::Error("没有缓存密码：请先手动连接一次该主机".into())
        };
        self.hosts.push(HostRun {
            name,
            node_id,
            handle,
            state,
        });
    }

    pub fn push_error(&mut self, name: String, node_id: String, message: String) {
        self.hosts.push(HostRun {
            name,
            node_id,
            handle: None,
            state: HostState::Error(message),
        });
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for host in &mut self.hosts {
            let mut next_state = None;
            if let Some(handle) = host.handle.as_mut() {
                for event in handle.drain() {
                    match event {
                        ExecEvent::Done { code, output } => {
                            next_state = Some(HostState::Done { code, output });
                        }
                        ExecEvent::Error(message) => {
                            next_state = Some(HostState::Error(message));
                        }
                    }
                }
                if next_state.is_some() {
                    handle.mark_finished();
                }
            }
            if let Some(state) = next_state {
                host.state = state;
                changed = true;
            }
        }
        changed
    }

    pub fn running(&self) -> bool {
        self.hosts
            .iter()
            .any(|host| matches!(host.state, HostState::Running))
    }

    pub fn stop(&mut self, index: usize) {
        if let Some(host) = self.hosts.get_mut(index) {
            if matches!(host.state, HostState::Running) {
                if let Some(handle) = &host.handle {
                    handle.close();
                }
                host.state = HostState::Stopped;
            }
        }
    }

    pub fn stop_all(&mut self) {
        for index in 0..self.hosts.len() {
            self.stop(index);
        }
    }
}
