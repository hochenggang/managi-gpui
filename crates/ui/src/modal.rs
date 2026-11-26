//! 弹窗状态：节点表单、单行输入（分组命名）、确认框。

use gpui_kit::component::input::InputState;
use gpui_kit::*;
use managi_core::{AuthKind, Node};

use crate::i18n::Strings;
use crate::state::new_node_id;

pub(crate) struct NodeForm {
    pub node_id: String,
    pub editing: bool,
    pub name: Entity<InputState>,
    pub host: Entity<InputState>,
    pub port: Entity<InputState>,
    pub username: Entity<InputState>,
    pub group: Entity<InputState>,
    pub auth: AuthKind,
    pub key_path: Entity<InputState>,
    pub error: Option<String>,
}

impl NodeForm {
    pub(crate) fn create(
        node: Option<&Node>,
        window: &mut Window,
        cx: &mut App,
        strings: Strings,
    ) -> Self {
        let node_id = node
            .map(|node| node.id.clone())
            .unwrap_or_else(new_node_id);

        let name = cx.new(|cx| {
            let input = InputState::new(window, cx).placeholder(strings.field_name_placeholder);
            match node {
                Some(node) => input.default_value(node.name.clone()),
                None => input,
            }
        });
        let host = cx.new(|cx| {
            let input = InputState::new(window, cx).placeholder(strings.field_host_placeholder);
            match node {
                Some(node) => input.default_value(node.host.clone()),
                None => input,
            }
        });
        let port = cx.new(|cx| {
            let value = node.map(|node| node.port.to_string()).unwrap_or_else(|| "22".into());
            InputState::new(window, cx)
                .placeholder(strings.field_port_placeholder)
                .default_value(value)
        });
        let username = cx.new(|cx| {
            let value = node
                .map(|node| node.username.clone())
                .unwrap_or_else(|| "root".into());
            InputState::new(window, cx)
                .placeholder(strings.field_user_placeholder)
                .default_value(value)
        });
        let group = cx.new(|cx| {
            let input = InputState::new(window, cx).placeholder(strings.field_group_placeholder);
            match node {
                Some(node) if !node.group.is_empty() => input.default_value(node.group.clone()),
                _ => input,
            }
        });
        let key_path = cx.new(|cx| {
            let input =
                InputState::new(window, cx).placeholder(strings.key_path_placeholder);
            match node {
                Some(node) if !node.key_path.is_empty() => input.default_value(node.key_path.clone()),
                _ => input,
            }
        });

        Self {
            node_id,
            editing: node.is_some(),
            name,
            host,
            port,
            username,
            group,
            auth: node.map(|node| node.auth).unwrap_or_default(),
            key_path,
            error: None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum PromptAction {
    NewGroup,
    RenameGroup(String),
    ConnectNode(String),
}

pub(crate) struct PromptModal {
    pub title: String,
    pub input: Entity<InputState>,
    pub action: PromptAction,
    pub error: Option<String>,
}

impl PromptModal {
    pub(crate) fn create(
        title: impl Into<String>,
        initial: Option<String>,
        action: PromptAction,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let title = title.into();
        let input = cx.new(|cx| {
            let input = InputState::new(window, cx);
            match initial {
                Some(value) => input.default_value(value),
                None => input,
            }
        });
        Self {
            title,
            input,
            action,
            error: None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ConfirmAction {
    DeleteNode(String),
    DeleteGroup(String),
}

pub(crate) struct ConfirmModal {
    pub message: String,
    pub action: ConfirmAction,
    pub error: Option<String>,
}

pub(crate) enum Modal {
    Node(NodeForm),
    Prompt(PromptModal),
    Confirm(ConfirmModal),
}
