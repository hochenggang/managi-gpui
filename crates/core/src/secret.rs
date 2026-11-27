//! 系统钥匙串：密码与私钥口令只存平台安全存储，不写 config.toml。
//!
//! Windows 使用凭据管理器，macOS 使用钥匙串，Linux 使用 Secret Service。
//! 所有失败都降级为“没有记住”，不阻塞连接。

use keyring::Entry;

use crate::node::Node;

const SERVICE: &str = "managi-gpui";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretKind {
    Password,
    Passphrase,
}

impl SecretKind {
    fn account(self, node_id: &str) -> String {
        match self {
            SecretKind::Password => format!("password:{node_id}"),
            SecretKind::Passphrase => format!("passphrase:{node_id}"),
        }
    }
}

/// 读取已保存的密码/口令；没有或系统钥匙串不可用时返回 `None`。
pub fn get(node_id: &str, kind: SecretKind) -> Option<String> {
    let entry = Entry::new(SERVICE, &kind.account(node_id)).ok()?;
    entry.get_password().ok()
}

/// 保存密码/口令；失败时返回错误文案，由 UI 决定是否提示。
pub fn set(node_id: &str, kind: SecretKind, secret: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE, &kind.account(node_id)).map_err(|error| error.to_string())?;
    entry
        .set_password(secret)
        .map_err(|error| error.to_string())
}

pub fn delete(node_id: &str, kind: SecretKind) {
    if let Ok(entry) = Entry::new(SERVICE, &kind.account(node_id)) {
        let _ = entry.delete_credential();
    }
}

/// 删除节点时清理两类凭据。
pub fn forget_node(node: &Node) {
    delete(&node.id, SecretKind::Password);
    delete(&node.id, SecretKind::Passphrase);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_names_are_separated_by_kind() {
        assert_ne!(
            SecretKind::Password.account("ssh:web"),
            SecretKind::Passphrase.account("ssh:web")
        );
        assert_eq!(SecretKind::Password.account("n1"), "password:n1");
    }
}
