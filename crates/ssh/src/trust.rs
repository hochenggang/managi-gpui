//! 主机密钥信任：TOFU（首次记录）与严格校验两种模式共用同一份 known_hosts。

use std::path::Path;

use russh::keys::{PublicKey, known_hosts};
use thiserror::Error;

/// 校验 `host:port` 的公钥是否已记录。文件不存在时返回 `Ok(false)`。
pub fn is_known_host(
    host: &str,
    port: u16,
    key: &PublicKey,
    path: impl AsRef<Path>,
) -> Result<bool, TrustError> {
    Ok(known_hosts::check_known_hosts_path(host, port, key, path)?)
}

/// 首次连接时记录公钥（OpenSSH known_hosts 格式，可被 `ssh` 复用）。
pub fn remember_host(
    host: &str,
    port: u16,
    key: &PublicKey,
    path: impl AsRef<Path>,
) -> Result<(), TrustError> {
    known_hosts::learn_known_hosts_path(host, port, key, path)?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum TrustError {
    #[error("读写 known_hosts 失败: {0}")]
    KnownHosts(#[from] russh::keys::Error),
}
