//! 无窗口自检：CI 在三平台直接运行，覆盖配置读写、终端解析、参数校验。

use std::error::Error;

use managi_core::{AppPaths, Config, Node};
use managi_term::{TerminalModel, TerminalSize};

pub fn run() -> Result<(), Box<dyn Error>> {
    println!("managi-gpui {} selftest", env!("CARGO_PKG_VERSION"));

    let root = std::env::temp_dir().join(format!("managi-gpui-selftest-{}", std::process::id()));
    let paths = AppPaths::with_root(&root);
    paths.ensure()?;

    let mut config = Config::default();
    config
        .nodes
        .push(Node::new("selftest", "自检节点", "127.0.0.1", 22, "root"));
    config.save(paths.config_file())?;

    let loaded = Config::load(paths.config_file())?;
    if loaded.nodes.len() != 1 {
        return Err(io_error("配置读写自检失败：节点数量不一致"));
    }

    let mut term = TerminalModel::new(TerminalSize::new(40, 6), 1000);
    term.advance(b"hello\x1b[31m red\x1b[0m\r\nworld");
    let screen = term.screen_text();
    if !screen.contains("hello red") || !screen.contains("world") {
        return Err(io_error(format!("终端解析自检失败：{screen:?}")));
    }

    let target = managi_ssh::SshTarget::new("127.0.0.1", 22, "root")?;
    if target.authority() != "127.0.0.1:22" {
        return Err(io_error("SSH 目标校验自检失败"));
    }

    println!("config: {}", paths.config_file().display());
    println!("screen: {:?}", screen.lines().next().unwrap_or_default());
    println!("selftest ok");

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

fn io_error(message: impl Into<String>) -> std::io::Error {
    std::io::Error::other(message.into())
}
