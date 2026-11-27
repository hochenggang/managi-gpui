//! 领域模型与配置层。
//!
//! 这里不出现任何 UI、网络、异步运行时类型：`ui` 与 `ssh` 都依赖本 crate，
//! 但本 crate 不反向依赖它们，保证依赖方向单向。

pub mod config;
pub mod node;
pub mod paths;
pub mod secret;
pub mod settings;
pub mod ssh_config;

pub use config::{Config, ConfigError};
pub use node::{AuthKind, Node, NodeError, NodeSource};
pub use paths::{AppPaths, PathsError};
pub use settings::{Language, Settings, ThemeName};
