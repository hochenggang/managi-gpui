//! 应用数据目录解析。
//!
//! 三平台统一落在各自的用户配置目录下的 `managi-gpui/`：
//! - Windows: `%APPDATA%\managi-gpui`
//! - macOS: `~/Library/Application Support/managi-gpui`
//! - Linux: `~/.config/managi-gpui`

use std::path::{Path, PathBuf};

use directories::BaseDirs;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathsError {
    #[error("无法确定用户目录")]
    NoHomeDir,
    #[error("创建目录失败: {0}")]
    Io(#[from] std::io::Error),
}

/// 所有落盘位置的唯一来源；测试可用 [`AppPaths::with_root`] 指向临时目录。
#[derive(Clone, Debug)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    /// 按平台约定解析根目录。只算路径，不创建目录。
    pub fn discover() -> Result<Self, PathsError> {
        let base = BaseDirs::new().ok_or(PathsError::NoHomeDir)?;
        Ok(Self {
            root: base.config_dir().join("managi-gpui"),
        })
    }

    /// 显式指定根目录（测试 / 便携模式）。
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn known_hosts_file(&self) -> PathBuf {
        self.root.join("known_hosts")
    }

    pub fn log_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// 确保目录存在。
    pub fn ensure(&self) -> Result<(), PathsError> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.log_dir())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_root_maps_expected_files() {
        let paths = AppPaths::with_root("/tmp/managi-gpui-test");
        assert_eq!(paths.config_file(), PathBuf::from("/tmp/managi-gpui-test/config.toml"));
        assert_eq!(
            paths.known_hosts_file(),
            PathBuf::from("/tmp/managi-gpui-test/known_hosts")
        );
        assert_eq!(paths.log_dir(), PathBuf::from("/tmp/managi-gpui-test/logs"));
    }
}
