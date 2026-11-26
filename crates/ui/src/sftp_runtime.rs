//! SFTP 界面运行时：目录缓存、加载状态、传输进度。

use managi_ssh::{SftpCommand, SftpEntry, SftpEvent, SftpHandle};

pub struct SftpRuntime {
    pub handle: SftpHandle,
    pub home: String,
    pub path: String,
    pub entries: Vec<SftpEntry>,
    pub loading: bool,
    pub error: Option<String>,
    pub progress: Option<(String, u8)>,
    pub connected: bool,
    pub selected: Option<String>,
}

impl SftpRuntime {
    pub fn new(handle: SftpHandle) -> Self {
        Self {
            handle,
            home: String::new(),
            path: String::new(),
            entries: Vec::new(),
            loading: true,
            error: None,
            progress: None,
            connected: false,
            selected: None,
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for event in self.handle.drain() {
            changed = true;
            match event {
                SftpEvent::Ready { home } => {
                    self.connected = true;
                    self.home = home.clone();
                    self.path = home;
                    self.loading = true;
                    self.requested = true;
                    self.handle.send(SftpCommand::List(self.path.clone()));
                }
                SftpEvent::Entries { path, items } => {
                    self.path = path;
                    self.entries = items;
                    self.loading = false;
                    self.error = None;
                }
                SftpEvent::Progress { label, percent } => {
                    self.progress = Some((label, percent));
                }
                SftpEvent::Done { .. } => {
                    self.progress = None;
                    self.error = None;
                    self.refresh();
                }
                SftpEvent::Failed { label, message } => {
                    self.progress = None;
                    self.loading = false;
                    self.error = Some(if label.is_empty() {
                        message
                    } else {
                        format!("{label}: {message}")
                    });
                }
                SftpEvent::Closed(reason) => {
                    self.connected = false;
                    self.loading = false;
                    if self.error.is_none() {
                        self.error = Some(reason);
                    }
                }
            }
        }
        changed
    }

    pub fn refresh(&mut self) {
        if self.connected {
            self.loading = true;
            self.handle.send(SftpCommand::List(self.path.clone()));
        }
    }

    pub fn go(&mut self, path: String) {
        self.path = path;
        self.loading = true;
        self.entries.clear();
        self.selected = None;
        self.error = None;
        if self.connected {
            self.handle.send(SftpCommand::List(self.path.clone()));
        }
    }

    pub fn up(&mut self) {
        if self.path == "/" {
            return;
        }
        let parent = match self.path.rfind('/') {
            Some(0) | None => "/".to_string(),
            Some(index) => self.path[..index].to_string(),
        };
        self.go(parent);
    }

    pub fn open_dir(&mut self, name: &str) {
        self.go(join_path(&self.path, name));
    }

    pub fn download(&mut self, name: &str) {
        let remote = join_path(&self.path, name);
        let local = managi_ssh::download_root().join(name);
        self.handle.send(SftpCommand::Download { remote, local });
    }

    pub fn upload(&mut self, local: std::path::PathBuf) {
        let Some(name) = local
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            self.error = Some("本地路径缺少文件名".into());
            return;
        };
        let remote = join_path(&self.path, &name);
        self.handle.send(SftpCommand::Upload { local, remote });
    }

    pub fn delete(&mut self, entry: &SftpEntry) {
        self.handle.send(SftpCommand::Delete {
            path: join_path(&self.path, &entry.name),
            is_dir: entry.is_dir,
        });
    }

    pub fn mkdir(&mut self, name: &str) {
        self.handle
            .send(SftpCommand::Mkdir(join_path(&self.path, name)));
    }

    pub fn is_live(&self) -> bool {
        self.connected && !self.handle.is_closed()
    }
}

pub fn join_path(base: &str, name: &str) -> String {
    if base.is_empty() || base == "/" {
        format!("/{name}")
    } else {
        format!("{base}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_posix_paths() {
        assert_eq!(join_path("/", "etc"), "/etc");
        assert_eq!(join_path("/var/log", "syslog"), "/var/log/syslog");
    }
}
