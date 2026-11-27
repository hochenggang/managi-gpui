//! OpenSSH `~/.ssh/config` 的只读解析与 Managi 托管片段写入。
//!
//! 设计要点：
//! - 读取时展开 `Include`，按 OpenSSH 的「先匹配到的值优先」规则合并 `Host *` 默认项；
//! - 不重写用户原有 Host；新增/修改主机写入独立托管文件；
//! - 主配置只增加一行 `Include`，并且只在首次写入时增加。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::node::{AuthKind, Node, NodeSource};
use crate::paths::home_dir;

pub const SYSTEM_GROUP: &str = "SSH 配置";

#[derive(Clone, Debug)]
pub struct SshConfigPaths {
    pub main: PathBuf,
    pub managed: PathBuf,
}

#[derive(Debug, Error)]
pub enum SshConfigError {
    #[error("无法确定用户主目录")]
    NoHomeDir,
    #[error("读取 SSH 配置失败: {0}")]
    Io(#[from] std::io::Error),
}

/// 默认路径。若现有配置已 `Include ~/.ssh/config.d/*`，托管文件放进该目录。
pub fn default_paths() -> Result<SshConfigPaths, SshConfigError> {
    let home = home_dir().ok_or(SshConfigError::NoHomeDir)?;
    let ssh = home.join(".ssh");
    let main = ssh.join("config");
    let text = fs::read_to_string(&main).unwrap_or_default();
    let managed = if text.contains("config.d") {
        ssh.join("config.d").join("60-managi.conf")
    } else {
        ssh.join("managi").join("config")
    };
    Ok(SshConfigPaths { main, managed })
}

/// 读取全部系统主机（含托管文件）。用户 Host 与托管 Host 都标记来源。
pub fn read_hosts(paths: &SshConfigPaths) -> Result<Vec<Node>, SshConfigError> {
    let blocks = parse_all(&paths.main, &paths.managed)?;
    let mut aliases: Vec<String> = Vec::new();
    let mut seen = HashSet::new();

    for block in &blocks {
        for pattern in &block.patterns {
            if pattern.starts_with('!') || pattern.contains('*') || pattern.contains('?') {
                continue;
            }
            if pattern.contains('%') || !seen.insert(pattern.to_lowercase()) {
                continue;
            }
            aliases.push(pattern.clone());
        }
    }

    let mut nodes = Vec::new();
    for alias in aliases {
        let mut values = EffectiveValues::default();
        for block in &blocks {
            if matches_alias(&alias, &block.patterns) {
                if block.managed {
                    values.source_is_managed = true;
                }
                for (key, value) in &block.options {
                    values.set_once(key, value);
                }
            }
        }

        if values.has("proxyjump") || values.has("proxycommand") {
            // 代理链暂未实现，先不展示，避免点开必然失败。
            continue;
        }

        let host = values.get("hostname").unwrap_or(&alias).to_string();
        let username = values
            .get("user")
            .map(str::to_string)
            .or_else(|| std::env::var("USERNAME").ok())
            .or_else(|| std::env::var("USER").ok())
            .unwrap_or_else(|| "root".into());
        let port = values
            .get("port")
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|port| *port > 0)
            .unwrap_or(22);
        let key_path = values
            .get("identityfile")
            .map(|value| expand_path(value, &host, &username))
            .filter(|value| !value.is_empty());
        let source = if values.source_is_managed {
            NodeSource::Managed
        } else {
            NodeSource::SshConfig
        };

        let mut node = Node::new(format!("ssh:{alias}"), alias.clone(), host, port, username);
        node.alias = alias;
        node.group = SYSTEM_GROUP.into();
        node.source = source;
        if let Some(key_path) = key_path {
            node.auth = AuthKind::PrivateKey;
            node.key_path = key_path;
        } else {
            node.auth = AuthKind::Auto;
        }
        nodes.push(node);
    }
    Ok(nodes)
}

/// 确保主配置包含托管片段；返回是否实际修改了主配置。
pub fn ensure_include(paths: &SshConfigPaths) -> Result<bool, SshConfigError> {
    let include = format!("Include {}", display_path(&paths.managed));
    let existing = fs::read_to_string(&paths.main).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == include) {
        return Ok(false);
    }
    if paths.managed.components().any(|part| part.as_os_str() == "config.d") {
        // 该目录一般已被 `Include ~/.ssh/config.d/*` 覆盖，这里仍补一条精确 Include，
        // 保证即使没有目录通配也能生效。
        let dir_include = format!(
            "Include {}/*",
            display_path(paths.managed.parent().unwrap_or(Path::new("")))
        );
        if existing.lines().any(|line| line.trim() == dir_include) {
            return Ok(false);
        }
    }

    if let Some(parent) = paths.main.parent() {
        fs::create_dir_all(parent)?;
    }
    let backup = paths.main.with_extension("managi.bak");
    if paths.main.exists() && !backup.exists() {
        fs::copy(&paths.main, &backup)?;
    }
    let mut text = existing;
    if !text.ends_with('\n') && !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&include);
    text.push('\n');
    atomic_write(&paths.main, &text)?;
    Ok(true)
}

/// 写入或更新一个托管 Host 块。
pub fn upsert_managed_node(
    paths: &SshConfigPaths,
    node: &Node,
) -> Result<(), SshConfigError> {
    ensure_include(paths)?;
    let text = fs::read_to_string(&paths.managed).unwrap_or_default();
    let cleaned = remove_block(&text, &node.id);
    let mut next = cleaned;
    if !next.ends_with('\n') && !next.is_empty() {
        next.push('\n');
    }
    next.push_str(&render_block(node));
    atomic_write(&paths.managed, &next)
}

/// 删除一个托管 Host 块（只动带 Managi 标记的块）。
pub fn remove_managed_node(paths: &SshConfigPaths, id: &str) -> Result<(), SshConfigError> {
    let text = fs::read_to_string(&paths.managed).unwrap_or_default();
    atomic_write(&paths.managed, &remove_block(&text, id))
}

fn render_block(node: &Node) -> String {
    let alias = if node.alias.trim().is_empty() {
        node.name.trim()
    } else {
        node.alias.trim()
    };
    let mut block = format!("# managi:begin {}\nHost {}\n", node.id, alias);
    block.push_str(&format!("    HostName {}\n", node.host.trim()));
    if node.port != 22 {
        block.push_str(&format!("    Port {}\n", node.port));
    }
    block.push_str(&format!("    User {}\n", node.username.trim()));
    if node.uses_key() {
        block.push_str(&format!("    IdentityFile {}\n", quote(&node.key_path)));
    }
    block.push_str(&format!("# managi:end {}\n", node.id));
    block
}

fn remove_block(text: &str, id: &str) -> String {
    let begin = format!("# managi:begin {id}");
    let end = format!("# managi:end {id}");
    let mut output = String::new();
    let mut skipping = false;
    for line in text.lines() {
        if line.trim() == begin {
            skipping = true;
            continue;
        }
        if skipping {
            if line.trim() == end {
                skipping = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn atomic_write(path: &Path, content: &str) -> Result<(), SshConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("managi.tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if let Some(home) = home_dir() {
        let home_text = home.to_string_lossy().replace('\\', "/");
        if let Some(rest) = text.strip_prefix(&home_text) {
            return format!("~{rest}");
        }
    }
    text
}

fn quote(value: &str) -> String {
    if value.contains(' ') {
        format!("\"{value}\"")
    } else {
        value.to_string()
    }
}

fn expand_path(value: &str, host: &str, user: &str) -> String {
    let mut expanded = value.trim().trim_matches('"').to_string();
    if let Some(rest) = expanded.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            expanded = home.join(rest).to_string_lossy().to_string();
        }
    }
    expanded = expanded.replace("%d", &home_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default());
    expanded = expanded.replace("%h", host).replace("%u", user);
    expanded
}

#[derive(Default)]
struct EffectiveValues {
    values: Vec<(String, Vec<String>)>,
    source_is_managed: bool,
}

impl EffectiveValues {
    fn set_once(&mut self, key: &str, value: &str) {
        let key = key.to_ascii_lowercase();
        if !self.values.iter().any(|(existing, _)| existing == &key) {
            self.values.push((key, vec![value.to_string()]));
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(existing, _)| existing == key)
            .and_then(|(_, values)| values.first())
            .map(String::as_str)
    }

    fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

#[derive(Clone, Debug)]
struct Block {
    patterns: Vec<String>,
    options: Vec<(String, String)>,
    managed: bool,
}

fn parse_all(main: &Path, managed: &Path) -> Result<Vec<Block>, SshConfigError> {
    let mut blocks = Vec::new();
    let mut visited = HashSet::new();
    parse_file(main, managed, &mut blocks, &mut visited, 0)?;
    Ok(blocks)
}

fn parse_file(
    path: &Path,
    managed: &Path,
    blocks: &mut Vec<Block>,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<(), SshConfigError> {
    if depth > 8 {
        return Ok(());
    }
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical) {
        return Ok(());
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let is_managed = same_path(path, managed);

    let mut current: Option<Block> = None;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let (keyword, rest) = split_keyword(line);
        match keyword.to_ascii_lowercase().as_str() {
            "host" => {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }
                let patterns = rest.split_whitespace().map(str::to_string).collect();
                current = Some(Block {
                    patterns,
                    options: Vec::new(),
                    managed: is_managed,
                });
            }
            "match" => {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }
            }
            "include" => {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }
                for pattern in expand_include(rest, path.parent().unwrap_or(Path::new("."))) {
                    parse_file(&pattern, managed, blocks, visited, depth + 1)?;
                }
            }
            _ => {
                if let Some(block) = current.as_mut() {
                    block.options.push((
                        keyword.to_ascii_lowercase(),
                        rest.trim().to_string(),
                    ));
                }
            }
        }
    }
    if let Some(block) = current {
        blocks.push(block);
    }
    Ok(())
}

fn same_path(a: &Path, b: &Path) -> bool {
    fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf())
        == fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf())
}

fn expand_include(value: &str, base: &Path) -> Vec<PathBuf> {
    let mut value = value.trim().trim_matches('"').to_string();
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            value = home.join(rest).to_string_lossy().to_string();
        }
    }
    let path = PathBuf::from(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        base.join(path)
    };
    if !absolute.to_string_lossy().contains('*') {
        return vec![absolute];
    }
    let parent = absolute.parent().unwrap_or(Path::new("."));
    let name = absolute.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
    let prefix = name.split('*').next().unwrap_or("");
    let suffix = name.rsplit('*').next().unwrap_or("");
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with(prefix) && file_name.ends_with(suffix) {
                result.push(entry.path());
            }
        }
    }
    result.sort();
    result
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (index, ch) in line.char_indices() {
        if ch == '"' {
            quoted = !quoted;
        }
        if ch == '#' && !quoted {
            let before = &line[..index];
            let at_boundary = before.is_empty()
                || before.chars().last().map(|c| c.is_whitespace()).unwrap_or(true);
            if at_boundary {
                return &line[..index];
            }
        }
    }
    line
}

fn split_keyword(line: &str) -> (&str, &str) {
    let index = line
        .find(|ch: char| ch.is_whitespace() || ch == '=')
        .unwrap_or(line.len());
    let keyword = &line[..index];
    let rest = line[index..].trim_start_matches(|ch: char| ch.is_whitespace() || ch == '=');
    (keyword, rest)
}

fn matches_alias(alias: &str, patterns: &[String]) -> bool {
    let mut has_positive = false;
    let mut matched = false;
    for pattern in patterns {
        if let Some(negative) = pattern.strip_prefix('!') {
            if wildcard_match(negative, alias) {
                return false;
            }
        } else {
            has_positive = true;
            if wildcard_match(pattern, alias) {
                matched = true;
            }
        }
    }
    has_positive && matched
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let value: Vec<char> = value.to_lowercase().chars().collect();
    let mut dp = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;
    for i in 1..=pattern.len() {
        if pattern[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=pattern.len() {
        for j in 1..=value.len() {
            dp[i][j] = match pattern[i - 1] {
                '*' => dp[i - 1][j] || dp[i][j - 1],
                '?' => dp[i - 1][j - 1],
                ch => dp[i - 1][j - 1] && ch == value[j - 1],
            };
        }
    }
    dp[pattern.len()][value.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("managi-ssh-{}-{seq}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_hosts_defaults_and_first_value_wins() {
        let dir = temp_dir();
        let main = dir.join("config");
        fs::write(
            &main,
            "Host *\n  User default\n  IdentityFile ~/.ssh/id_ed25519\n\nHost web web2\n  HostName 10.0.0.1\n  User root\n  Port 2222\n",
        )
        .unwrap();
        let paths = SshConfigPaths {
            main: main.clone(),
            managed: dir.join("managi/config"),
        };
        let nodes = read_hosts(&paths).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "web");
        assert_eq!(nodes[0].host, "10.0.0.1");
        assert_eq!(nodes[0].port, 2222);
        assert_eq!(nodes[0].username, "default");
        assert!(nodes[0].key_path.ends_with("id_ed25519"));
        assert_eq!(nodes[1].name, "web2");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn writes_managed_block_and_enforces_include() {
        let dir = temp_dir();
        let main = dir.join("config");
        fs::write(&main, "Host existing\n  HostName 1.2.3.4\n").unwrap();
        let paths = SshConfigPaths {
            main: main.clone(),
            managed: dir.join("managi/config"),
        };
        let mut node = Node::new("n1", "测试", "5.6.7.8", 22, "root");
        node.alias = "test".into();
        upsert_managed_node(&paths, &node).unwrap();
        let text = fs::read_to_string(&main).unwrap();
        assert!(text.contains("Include "));
        assert!(text.contains("managi/config"));
        let managed = fs::read_to_string(&paths.managed).unwrap();
        assert!(managed.contains("Host test"));
        assert!(managed.contains("HostName 5.6.7.8"));
        upsert_managed_node(&paths, &node).unwrap();
        let managed = fs::read_to_string(&paths.managed).unwrap();
        assert_eq!(managed.matches("# managi:begin n1").count(), 1);
        remove_managed_node(&paths, "n1").unwrap();
        assert!(!fs::read_to_string(&paths.managed).unwrap().contains("Host test"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn wildcard_match_is_case_insensitive() {
        assert!(wildcard_match("web*", "Web1"));
        assert!(wildcard_match("*", "any"));
        assert!(!wildcard_match("web?", "web12"));
    }
}
