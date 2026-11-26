//! 连接目标的纯数据描述与校验。

use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshTarget {
    host: String,
    port: u16,
    username: String,
}

impl SshTarget {
    pub fn new(
        host: impl Into<String>,
        port: u16,
        username: impl Into<String>,
    ) -> Result<Self, TargetError> {
        let host = host.into().trim().to_string();
        let username = username.into().trim().to_string();

        if host.is_empty() {
            return Err(TargetError::EmptyHost);
        }
        if port == 0 {
            return Err(TargetError::InvalidPort);
        }
        if username.is_empty() {
            return Err(TargetError::EmptyUsername);
        }

        Ok(Self {
            host,
            port,
            username,
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    /// `host:port`，用于状态栏与日志。
    pub fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TargetError {
    #[error("主机地址不能为空")]
    EmptyHost,
    #[error("端口必须在 1-65535 之间")]
    InvalidPort,
    #[error("用户名不能为空")]
    EmptyUsername,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_validates() {
        let target = SshTarget::new("  10.0.0.1 ", 22, " root ").expect("valid target");
        assert_eq!(target.host(), "10.0.0.1");
        assert_eq!(target.username(), "root");
        assert_eq!(target.authority(), "10.0.0.1:22");
    }

    #[test]
    fn rejects_invalid_input() {
        assert_eq!(SshTarget::new("", 22, "root"), Err(TargetError::EmptyHost));
        assert_eq!(SshTarget::new("h", 0, "root"), Err(TargetError::InvalidPort));
        assert_eq!(SshTarget::new("h", 22, " "), Err(TargetError::EmptyUsername));
    }
}
