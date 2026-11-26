//! `config.toml` 的读写。
//!
//! 只保存非敏感信息；密码/私钥口令由钥匙串承担（见 PLAN §4.5）。
//! 写入采用「临时文件 + 原子替换」，避免断电/崩溃留下半截配置。

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::node::{Node, NodeError};
use crate::settings::Settings;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub nodes: Vec<Node>,
    pub groups: Vec<String>,
    pub settings: Settings,
}

impl Config {
    /// 文件不存在时返回默认配置；存在但损坏时报错，避免静默丢数据。
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(ConfigError::Io(error)),
        };

        let mut config: Config =
            toml::from_str(&text).map_err(|error| ConfigError::Parse(error.to_string()))?;
        config.settings.normalize();
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        self.validate()?;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let text =
            toml::to_string_pretty(self).map_err(|error| ConfigError::Serialize(error.to_string()))?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        for node in &self.nodes {
            node.validate()?;
        }

        let mut seen: Vec<&str> = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            if seen.contains(&node.id.as_str()) {
                return Err(ConfigError::DuplicateId(node.id.clone()));
            }
            seen.push(&node.id);
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("读写配置失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("解析配置失败: {0}")]
    Parse(String),
    #[error("序列化配置失败: {0}")]
    Serialize(String),
    #[error("节点配置无效: {0}")]
    Node(#[from] NodeError),
    #[error("节点标识重复: {0}")]
    DuplicateId(String),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::node::AuthKind;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_file(tag: &str) -> PathBuf {
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("managi-core-{}-{seq}", std::process::id()))
            .join(format!("{tag}.toml"))
    }

    fn sample() -> Config {
        let mut node = Node::new("n1", "测试机", "10.0.0.1", 2222, "root");
        node.auth = AuthKind::PrivateKey;
        node.group = "生产".into();
        Config {
            nodes: vec![node],
            groups: vec!["生产".into()],
            settings: Settings::default(),
        }
    }

    #[test]
    fn missing_file_yields_default() {
        let path = temp_file("missing");
        let config = Config::load(&path).expect("missing file is not an error");
        assert!(config.nodes.is_empty());
    }

    #[test]
    fn roundtrip_keeps_fields() {
        let path = temp_file("roundtrip");
        let config = sample();
        config.save(&path).expect("save");
        let loaded = Config::load(&path).expect("load");
        assert_eq!(loaded.nodes, config.nodes);
        assert_eq!(loaded.groups, config.groups);
        assert_eq!(loaded.nodes[0].auth, AuthKind::PrivateKey);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let mut config = sample();
        config.nodes.push(config.nodes[0].clone());
        assert!(matches!(config.validate(), Err(ConfigError::DuplicateId(_))));
    }

    #[test]
    fn invalid_port_is_rejected() {
        let mut config = sample();
        config.nodes[0].port = 0;
        assert!(config.validate().is_err());
    }
}
