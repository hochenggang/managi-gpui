//! 主机节点模型。

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 认证方式。密码/口令本身不落在这里，见 PLAN §4.5 的钥匙串设计。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// 有私钥就用私钥，否则走密码/钥匙串。
    #[default]
    Auto,
    Password,
    PrivateKey,
}

/// 节点来源。系统 SSH 配置里的主机只读；托管文件里的主机可编辑。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeSource {
    #[default]
    Local,
    SshConfig,
    Managed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// 稳定标识，用于钥匙串账户名与标签去重。
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub auth: AuthKind,
    /// 私钥文件路径；仅存路径，不复制内容。
    #[serde(default)]
    pub key_path: String,
    /// 分组名；空串表示未分组。
    #[serde(default)]
    pub group: String,
    /// OpenSSH Host 别名；本地节点为空。
    #[serde(default)]
    pub alias: String,
    /// 来源：本地配置 / 系统 ssh config / Managi 托管片段。
    #[serde(default)]
    pub source: NodeSource,
}

fn default_port() -> u16 {
    22
}

impl Node {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        username: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            host: host.into(),
            port,
            username: username.into(),
            auth: AuthKind::Auto,
            key_path: String::new(),
            group: String::new(),
            alias: String::new(),
            source: NodeSource::Local,
        }
    }

    /// 当前节点是否应走私钥认证（Auto 只要填了路径也走私钥）。
    pub fn uses_key(&self) -> bool {
        self.auth != AuthKind::Password && !self.key_path.trim().is_empty()
    }

    /// 入库前校验。UI 只展示 [`NodeError`] 的中文文案。
    pub fn validate(&self) -> Result<(), NodeError> {        if self.id.trim().is_empty() {
            return Err(NodeError::EmptyId);
        }
        if self.name.trim().is_empty() {
            return Err(NodeError::EmptyName);
        }
        if self.host.trim().is_empty() {
            return Err(NodeError::EmptyHost);
        }
        if self.port == 0 {
            return Err(NodeError::InvalidPort);
        }
        if self.username.trim().is_empty() {
            return Err(NodeError::EmptyUsername);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NodeError {
    #[error("节点标识不能为空")]
    EmptyId,
    #[error("备注名不能为空")]
    EmptyName,
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
    fn validates_minimal_node() {
        let node = Node::new("n1", "测试机", "10.0.0.1", 22, "root");
        assert_eq!(node.validate(), Ok(()));
    }

    #[test]
    fn rejects_empty_fields() {
        let mut node = Node::new("n1", "", "10.0.0.1", 22, "root");
        assert_eq!(node.validate(), Err(NodeError::EmptyName));

        node.name = "ok".into();
        node.port = 0;
        assert_eq!(node.validate(), Err(NodeError::InvalidPort));
    }
}
