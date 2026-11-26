//! 根视图：布局装配、右键菜单、弹窗，以及所有会改配置的动作。

use std::collections::HashMap;
use std::path::PathBuf;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;
use managi_core::{Config, Node};
use managi_ssh::{AuthMethod, ConnectRequest, SessionHandle};
use managi_term::encode_key;

use crate::i18n::Strings;
use crate::menu::{MenuAction, MenuItem, MenuState};
use crate::modal::{ConfirmAction, ConfirmModal, Modal, NodeForm, PromptAction, PromptModal};
use crate::state::{AppState, TabKind};
use crate::terminal_runtime::TerminalRuntime;
use crate::theme::{self, Palette};
use crate::widgets::text_input;

const SIDEBAR_WIDTH: f32 = 320.0;
const SIDEBAR_COLLAPSED: f32 = 192.0;
pub(crate) const BAR_HEIGHT: f32 = 32.0;

pub struct AppView {
    pub(crate) state: AppState,
    pub(crate) search: Entity<InputState>,
    pub(crate) modal: Option<Modal>,
    pub(crate) menu: Option<MenuState>,
    pub(crate) terminals: HashMap<String, TerminalRuntime>,
    pub(crate) passwords: HashMap<String, String>,
    pub(crate) term_focus: FocusHandle,
}

impl AppView {
    pub fn new(
        config: Config,
        config_path: PathBuf,
        version: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let state = AppState::new(config, config_path, version.into());
        let strings = state.strings();
        let palette = theme::palette(state.config.settings.theme);
        theme::apply_to_component(cx, palette, state.config.settings.terminal_font_size);

        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(strings.search_placeholder)
        });

        cx.subscribe_in(&search, window, |this, state, event, _window, cx| {
            if matches!(event, InputEvent::Change) {
                this.state.query = state.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();

        Self {
            state,
            search,
            modal: None,
            menu: None,
            terminals: HashMap::new(),
            passwords: HashMap::new(),
            term_focus: cx.focus_handle(),
        }
    }

    pub(crate) fn open_terminal(
        &mut self,
        node: &Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.open_node_tab(TabKind::Terminal, node);
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        if self.terminals.contains_key(&tab_id) {
            cx.notify();
            return;
        }

        if node.auth == managi_core::AuthKind::PrivateKey && !node.key_path.trim().is_empty() {
            self.start_session(&tab_id, node, None, window, cx);
        } else if let Some(password) = self.passwords.get(&node.id).cloned() {
            self.start_session(&tab_id, node, Some(password), window, cx);
        } else {
            let strings = self.state.strings();
            self.open_prompt(
                format!("{} {}", strings.connect_prefix, node.name),
                None,
                PromptAction::ConnectNode(node.id.clone()),
                window,
                cx,
            );
        }
    }

    fn start_session(
        &mut self,
        tab_id: &str,
        node: &Node,
        password: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let auth = match node.auth {
            managi_core::AuthKind::Password => {
                let Some(password) = password else {
                    return;
                };
                AuthMethod::Password(password)
            }
            managi_core::AuthKind::PrivateKey => AuthMethod::PrivateKey {
                path: PathBuf::from(node.key_path.trim()),
                passphrase: self.passwords.get(&node.id).cloned(),
            },
        };

        let known_hosts_path = self
            .state
            .config_path
            .parent()
            .map(|parent| parent.join("known_hosts"))
            .unwrap_or_else(|| PathBuf::from("known_hosts"));

        let request = ConnectRequest {
            host: node.host.clone(),
            port: node.port,
            username: node.username.clone(),
            auth,
            known_hosts_path,
            columns: 80,
            rows: 24,
        };

        match managi_ssh::session::spawn(request) {
            Ok(handle) => {
                self.terminals.insert(
                    tab_id.to_string(),
                    TerminalRuntime::new(handle, 80, 24),
                );
            }
            Err(error) => {
                self.state.notice = Some(error.to_string());
            }
        }
        self.term_focus.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn reconnect_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let node = self
            .state
            .active_tab()
            .and_then(|tab| tab.node.clone());
        let Some(node) = node else {
            return;
        };
        self.terminals.remove(&tab_id);

        if node.auth == managi_core::AuthKind::PrivateKey && !node.key_path.trim().is_empty() {
            self.start_session(&tab_id, &node, None, window, cx);
        } else if let Some(password) = self.passwords.get(&node.id).cloned() {
            self.start_session(&tab_id, &node, Some(password), window, cx);
        } else {
            let strings = self.state.strings();
            self.open_prompt(
                format!("{} {}", strings.connect_prefix, node.name),
                None,
                PromptAction::ConnectNode(node.id.clone()),
                window,
                cx,
            );
        }
    }

    pub(crate) fn handle_terminal_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let control = keystroke.modifiers.control;
        let shift = keystroke.modifiers.shift;

        if control && shift && key.eq_ignore_ascii_case("c") {
            if let Some(runtime) = self.terminals.get(&tab_id) {
                let text = runtime.model.screen_text();
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            return;
        }
        if control && shift && key.eq_ignore_ascii_case("v") {
            let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
                return;
            };
            let Some(runtime) = self.terminals.get_mut(&tab_id) else {
                return;
            };
            if !runtime.is_live() {
                return;
            }
            let mut payload = text.replace("\r\n", "\n").replace('\n', "\r").into_bytes();
            if runtime.bracketed_paste() {
                let mut wrapped = Vec::with_capacity(payload.len() + 12);
                wrapped.extend_from_slice(b"\x1b[200~");
                wrapped.append(&mut payload);
                wrapped.extend_from_slice(b"\x1b[201~");
                runtime.handle.write(wrapped);
            } else {
                runtime.handle.write(payload);
            }
            cx.notify();
            return;
        }
        if shift && key.eq_ignore_ascii_case("pageup") {
            self.scroll_active(8, cx);
            return;
        }
        if shift && key.eq_ignore_ascii_case("pagedown") {
            self.scroll_active(-8, cx);
            return;
        }

        let Some(runtime) = self.terminals.get_mut(&tab_id) else {
            return;
        };
        if !runtime.is_live() {
            return;
        }
        let keystroke = &event.keystroke;
        let mode = runtime.model.mode();
        if let Some(bytes) = encode_key(
            &keystroke.key,
            keystroke.key_char.as_deref(),
            keystroke.modifiers.control,
            keystroke.modifiers.alt,
            keystroke.modifiers.shift,
            mode,
        ) {
            runtime.handle.write(bytes);
        }
        cx.notify();
    }

    pub(crate) fn scroll_active(&mut self, lines: i32, cx: &mut Context<Self>) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        if let Some(runtime) = self.terminals.get_mut(&tab_id) {
            runtime.scroll(lines);
            cx.notify();
        }
    }

    fn poll_terminals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active_tab = self.state.active_tab.clone();
        let active_is_terminal = self
            .state
            .active_tab()
            .is_some_and(|tab| tab.kind == TabKind::Terminal);

        let valid_tabs: Vec<String> = self
            .state
            .tabs
            .iter()
            .map(|tab| tab.id.clone())
            .collect();
        self.terminals
            .retain(|tab_id, _| valid_tabs.iter().any(|id| id == tab_id));

        if self.terminals.is_empty() {
            return;
        }

        let font_size = f32::from(self.state.config.settings.terminal_font_size);
        let width = window.bounds().size.width.as_f32()
            - sidebar_width(self.state.sidebar_collapsed)
            - 16.0;
        let height = window.bounds().size.height.as_f32() - BAR_HEIGHT * 2.0 - 16.0;
        let columns = ((width / (font_size * 0.62)).floor() as u16).max(20);
        let rows = ((height / (font_size * 1.28)).floor() as u16).max(4);

        let mut any_live = false;
        for (tab_id, runtime) in &mut self.terminals {
            let changed = runtime.poll();
            if active_is_terminal && active_tab.as_deref() == Some(tab_id.as_str()) {
                runtime.resize(columns, rows);
            }
            if runtime.is_live() {
                any_live = true;
            }
            if changed && active_is_terminal && active_tab.as_deref() == Some(tab_id.as_str()) {
                cx.notify();
            }
        }
        // 会话不断产生输出；仍有活动会话时持续请求下一帧。
        if any_live {
            window.request_animation_frame();
        }
    }

    pub(crate) fn close_tab(&mut self, id: &str, cx: &mut Context<Self>) {
        self.terminals.remove(id);
        self.state.close_tab(id);
        cx.notify();
    }

    pub(crate) fn open_node_form(
        &mut self,
        node_id: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.state.strings();
        let node = node_id.and_then(|id| {
            self.state
                .config
                .nodes
                .iter()
                .find(|node| node.id == id)
                .cloned()
        });
        let form = NodeForm::create(node.as_ref(), window, cx, strings);
        let handle = form.name.read(cx).focus_handle(cx);
        handle.focus(window, cx);
        self.modal = Some(Modal::Node(form));
        cx.notify();
    }

    fn open_prompt(
        &mut self,
        title: String,
        initial: Option<String>,
        action: PromptAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let masked = matches!(&action, PromptAction::ConnectNode(_));
        let prompt = PromptModal::create(title, initial, action, masked, window, cx);
        let handle = prompt.input.read(cx).focus_handle(cx);
        handle.focus(window, cx);
        self.modal = Some(Modal::Prompt(prompt));
        cx.notify();
    }

    fn open_confirm(&mut self, message: String, action: ConfirmAction, cx: &mut Context<Self>) {
        self.modal = Some(Modal::Confirm(ConfirmModal {
            message,
            action,
            error: None,
        }));
        cx.notify();
    }

    fn set_modal_error(&mut self, error: String) {
        match &mut self.modal {
            Some(Modal::Node(form)) => form.error = Some(error),
            Some(Modal::Prompt(prompt)) => prompt.error = Some(error),
            Some(Modal::Confirm(confirm)) => confirm.error = Some(error),
            None => {}
        }
    }

    fn save_node(&mut self, cx: &mut Context<Self>) {
        let strings = self.state.strings();
        let Some(Modal::Node(form)) = &self.modal else {
            return;
        };

        let node_id = form.node_id.clone();
        let editing = form.editing;
        let name = form.name.read(cx).value().trim().to_string();
        let host = form.host.read(cx).value().trim().to_string();
        let port_text = form.port.read(cx).value().trim().to_string();
        let username = form.username.read(cx).value().trim().to_string();
        let group = form.group.read(cx).value().trim().to_string();
        let auth = form.auth;
        let key_path = form.key_path.read(cx).value().trim().to_string();

        if name.is_empty() {
            self.set_modal_error(strings.empty_name.to_string());
            cx.notify();
            return;
        }
        if host.is_empty() {
            self.set_modal_error(strings.empty_host.to_string());
            cx.notify();
            return;
        }
        if username.is_empty() {
            self.set_modal_error(strings.empty_user.to_string());
            cx.notify();
            return;
        }
        if auth == managi_core::AuthKind::PrivateKey && key_path.is_empty() {
            self.set_modal_error(strings.key_path_required.to_string());
            cx.notify();
            return;
        }
        let port = match port_text.parse::<u16>() {
            Ok(port) if port > 0 => port,
            _ => {
                self.set_modal_error(strings.invalid_port.to_string());
                cx.notify();
                return;
            }
        };

        let mut node = Node::new(node_id.clone(), name, host, port, username);
        node.group = group.clone();
        node.auth = auth;
        node.key_path = key_path;

        if editing {
            match self
                .state
                .config
                .nodes
                .iter_mut()
                .find(|existing| existing.id == node_id)
            {
                Some(existing) => *existing = node,
                None => self.state.config.nodes.push(node),
            }
        } else {
            self.state.config.nodes.push(node);
        }

        if !group.is_empty() && !self.state.config.groups.iter().any(|name| name == &group) {
            self.state.config.groups.push(group);
        }

        match self.state.persist() {
            Ok(()) => {
                self.modal = None;
                cx.notify();
            }
            Err(error) => {
                self.set_modal_error(format!("{}: {error}", strings.save_failed));
                cx.notify();
            }
        }
    }

    fn save_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.state.strings();
        let Some(Modal::Prompt(prompt)) = &self.modal else {
            return;
        };
        let value = prompt.input.read(cx).value().trim().to_string();
        let action = prompt.action.clone();

        if value.is_empty() {
            let message = match &action {
                PromptAction::ConnectNode(_) => strings.password_required,
                _ => strings.name_required,
            };
            self.set_modal_error(message.to_string());
            cx.notify();
            return;
        }

        match action {
            PromptAction::ConnectNode(node_id) => {
                self.passwords.insert(node_id.clone(), value);
                let node = self
                    .state
                    .config
                    .nodes
                    .iter()
                    .find(|node| node.id == node_id)
                    .cloned();
                self.modal = None;
                if let Some(node) = node {
                    let Some(tab_id) = self.state.active_tab.clone() else {
                        return;
                    };
                    self.start_session(&tab_id, &node, self.passwords.get(&node_id).cloned(), window, cx);
                }
                cx.notify();
                return;
            }
            PromptAction::NewGroup => {
                if self.state.config.groups.iter().any(|name| name == &value) {
                    self.set_modal_error(strings.group_exists.to_string());
                    cx.notify();
                    return;
                }
                self.state.config.groups.push(value);
            }
            PromptAction::RenameGroup(old) => {
                if old != value {
                    if self.state.config.groups.iter().any(|name| name == &value) {
                        self.set_modal_error(strings.group_exists.to_string());
                        cx.notify();
                        return;
                    }
                    if let Some(group) = self
                        .state
                        .config
                        .groups
                        .iter_mut()
                        .find(|name| name.as_str() == old.as_str())
                    {
                        *group = value.clone();
                    }
                    for node in &mut self.state.config.nodes {
                        if node.group == old {
                            node.group = value.clone();
                        }
                    }
                }
            }
        }

        match self.state.persist() {
            Ok(()) => {
                self.modal = None;
                cx.notify();
            }
            Err(error) => {
                self.set_modal_error(format!("{}: {error}", strings.save_failed));
                cx.notify();
            }
        }
    }

    fn apply_confirm(&mut self, cx: &mut Context<Self>) {
        let strings = self.state.strings();
        let Some(Modal::Confirm(confirm)) = &self.modal else {
            return;
        };
        let action = confirm.action.clone();

        match action {
            ConfirmAction::DeleteNode(id) => {
                self.state.config.nodes.retain(|node| node.id != id);
                self.state.selected_nodes.retain(|node| node != &id);
            }
            ConfirmAction::DeleteGroup(group) => {
                self.state.config.groups.retain(|name| name != &group);
                for node in &mut self.state.config.nodes {
                    if node.group == group {
                        node.group.clear();
                    }
                }
            }
        }

        match self.state.persist() {
            Ok(()) => {
                self.modal = None;
                cx.notify();
            }
            Err(error) => {
                self.set_modal_error(format!("{}: {error}", strings.save_failed));
                cx.notify();
            }
        }
    }

    pub(crate) fn perform(
        &mut self,
        action: MenuAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        match action {
            MenuAction::EditNode(id) => self.open_node_form(Some(&id), window, cx),
            MenuAction::DeleteNode(id) => {
                let strings = self.state.strings();
                let name = self
                    .state
                    .config
                    .nodes
                    .iter()
                    .find(|node| node.id == id)
                    .map(|node| node.name.clone())
                    .unwrap_or_default();
                let message = format!("{} {}?", strings.delete_node_confirm, name);
                self.open_confirm(message, ConfirmAction::DeleteNode(id), cx);
            }
            MenuAction::MoveNode { node, group } => {
                if let Some(target) = self
                    .state
                    .config
                    .nodes
                    .iter_mut()
                    .find(|entry| entry.id == node)
                {
                    target.group = group;
                }
                if let Err(error) = self.state.persist() {
                    self.state.notice = Some(error);
                }
                cx.notify();
            }
            MenuAction::NewGroup => {
                let strings = self.state.strings();
                self.open_prompt(
                    strings.new_group.to_string(),
                    None,
                    PromptAction::NewGroup,
                    window,
                    cx,
                );
            }
            MenuAction::RenameGroup(group) => {
                let strings = self.state.strings();
                self.open_prompt(
                    strings.rename_group.to_string(),
                    Some(group.clone()),
                    PromptAction::RenameGroup(group),
                    window,
                    cx,
                );
            }
            MenuAction::DeleteGroup(group) => {
                let strings = self.state.strings();
                let message = format!("{} {}?", strings.delete_group, group);
                self.open_confirm(message, ConfirmAction::DeleteGroup(group), cx);
            }
            MenuAction::CloseTab(id) => {
                self.close_tab(&id, cx);
            }
            MenuAction::CloseOthers(id) => {
                let keep = self.state.tabs.iter().find(|tab| tab.id == id);
                let keep_id = keep.map(|tab| tab.id.clone());
                self.terminals
                    .retain(|tab_id, _| keep_id.as_deref() == Some(tab_id.as_str()));
                self.state.close_others(&id);
                cx.notify();
            }
            MenuAction::CloseAll => {
                self.terminals.clear();
                self.state.close_all();
                cx.notify();
            }
        }
    }

    pub(crate) fn node_menu(&self, node_id: &str) -> Vec<MenuItem> {
        let strings = self.state.strings();
        let mut items = vec![MenuItem {
            label: strings.edit.to_string(),
            action: MenuAction::EditNode(node_id.to_string()),
            danger: false,
        }];

        items.push(MenuItem {
            label: strings.move_to_ungrouped.to_string(),
            action: MenuAction::MoveNode {
                node: node_id.to_string(),
                group: String::new(),
            },
            danger: false,
        });

        for group in &self.state.config.groups {
            items.push(MenuItem {
                label: format!("{} \"{}\"", strings.move_to_group, group),
                action: MenuAction::MoveNode {
                    node: node_id.to_string(),
                    group: group.clone(),
                },
                danger: false,
            });
        }

        items.push(MenuItem {
            label: strings.delete.to_string(),
            action: MenuAction::DeleteNode(node_id.to_string()),
            danger: true,
        });
        items
    }

    pub(crate) fn group_menu(&self, group: &str) -> Vec<MenuItem> {
        let strings = self.state.strings();
        if group.is_empty() {
            return vec![MenuItem {
                label: strings.new_group.to_string(),
                action: MenuAction::NewGroup,
                danger: false,
            }];
        }

        vec![
            MenuItem {
                label: strings.rename_group.to_string(),
                action: MenuAction::RenameGroup(group.to_string()),
                danger: false,
            },
            MenuItem {
                label: strings.delete_group.to_string(),
                action: MenuAction::DeleteGroup(group.to_string()),
                danger: true,
            },
        ]
    }

    pub(crate) fn tab_menu(&self, id: &str) -> Vec<MenuItem> {
        let strings = self.state.strings();
        vec![
            MenuItem {
                label: strings.close.to_string(),
                action: MenuAction::CloseTab(id.to_string()),
                danger: false,
            },
            MenuItem {
                label: strings.close_others.to_string(),
                action: MenuAction::CloseOthers(id.to_string()),
                danger: false,
            },
            MenuItem {
                label: strings.close_all.to_string(),
                action: MenuAction::CloseAll,
                danger: false,
            },
        ]
    }

    pub(crate) fn open_menu(
        &mut self,
        x: f32,
        y: f32,
        items: Vec<MenuItem>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let size = window.bounds().size;
        let max_x = (size.width.as_f32() - 180.0).max(0.0);
        let max_y = (size.height.as_f32() - 200.0).max(0.0);
        self.menu = Some(MenuState {
            x: x.min(max_x),
            y: y.min(max_y),
            items,
        });
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.poll_terminals(window, cx);
        let palette = theme::palette(self.state.config.settings.theme);
        let strings = self.state.strings();

        let sidebar = crate::sidebar::render(&self.state, &self.search, &strings, palette, window, cx);
        let tabs = crate::tab_bar::render(&self.state, &strings, palette, cx);
        let workspace = crate::workspace::render(
            &self.state,
            &strings,
            palette,
            &self.terminals,
            &self.term_focus,
            cx,
        );

        let mut root = div()
            .relative()
            .size_full()
            .flex()
            .flex_row()
            .bg(palette.bg)
            .text_color(palette.font_1)
            .text_size(px(11.2))
            .font(theme::ui_font())
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .child(tabs)
                    .child(workspace),
            );

        if let Some(modal) = &self.modal {
            root = root.child(modal_layer(modal, &strings, palette, window, cx));
        }
        if let Some(menu) = &self.menu {
            root = root.child(menu_layer(menu, palette, cx));
        }
        root
    }
}

fn modal_layer(
    modal: &Modal,
    strings: &Strings,
    palette: Palette,
    window: &Window,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let panel = match modal {
        Modal::Node(form) => node_form_panel(form, strings, palette, window, cx),
        Modal::Prompt(prompt) => prompt_panel(prompt, strings, palette, window, cx),
        Modal::Confirm(confirm) => confirm_panel(confirm, strings, palette, cx),
    };

    div()
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x00000059))
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
            this.modal = None;
            cx.notify();
        }))
        .child(
            panel
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        )
        .into_any_element()
}

fn node_form_panel(
    form: &NodeForm,
    strings: &Strings,
    palette: Palette,
    window: &Window,
    cx: &mut Context<AppView>,
) -> Div {
    let title = if form.editing {
        strings.node_edit_title
    } else {
        strings.node_new_title
    };

    let mut panel = div()
        .w(px(420.0))
        .flex()
        .flex_col()
        .p(px(16.0))
        .bg(palette.panel_bg)
        .border_1()
        .border_color(palette.border)
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.font_1)
                .mb(px(12.0))
                .child(title.to_string()),
        )
        .child(field(
            strings.field_name,
            text_input(&form.name, palette, 32.0, 14.4, window, cx),
            palette,
        ))
        .child(field(
            strings.field_host,
            text_input(&form.host, palette, 32.0, 14.4, window, cx),
            palette,
        ))
        .child(field(
            strings.field_port,
            text_input(&form.port, palette, 32.0, 14.4, window, cx),
            palette,
        ))
        .child(field(
            strings.field_user,
            text_input(&form.username, palette, 32.0, 14.4, window, cx),
            palette,
        ))
        .child(field(
            strings.auth_type,
            div()
                .flex()
                .gap(px(6.0))
                .child(auth_choice(
                    "auth-password",
                    strings.auth_password,
                    form.auth == managi_core::AuthKind::Password,
                    palette,
                    cx,
                ))
                .child(auth_choice(
                    "auth-key",
                    strings.auth_key,
                    form.auth == managi_core::AuthKind::PrivateKey,
                    palette,
                    cx,
                )),
            palette,
        ))
        .child(field(
            strings.field_group,
            text_input(&form.group, palette, 32.0, 14.4, window, cx),
            palette,
        ));

    if form.auth == managi_core::AuthKind::PrivateKey {
        panel = panel.child(field(
            strings.key_path,
            text_input(&form.key_path, palette, 32.0, 14.4, window, cx),
            palette,
        ));
    }

    if let Some(error) = &form.error {
        panel = panel.child(
            div()
                .text_size(px(11.2))
                .text_color(palette.red)
                .mb(px(8.0))
                .child(error.clone()),
        );
    }

    panel.child(
        div()
            .mt(px(4.0))
            .flex()
            .justify_end()
            .gap(px(8.0))
            .child(secondary_button(strings.cancel, palette).on_click(cx.listener(
                |this, _, _, cx| {
                    this.modal = None;
                    cx.notify();
                },
            )))
            .child(primary_button(strings.save, palette).on_click(cx.listener(
                |this, _, _, cx| this.save_node(cx),
            ))),
    )
}

fn prompt_panel(
    prompt: &PromptModal,
    strings: &Strings,
    palette: Palette,
    window: &Window,
    cx: &mut Context<AppView>,
) -> Div {
    let mut panel = div()
        .w(px(420.0))
        .flex()
        .flex_col()
        .p(px(16.0))
        .bg(palette.panel_bg)
        .border_1()
        .border_color(palette.border)
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.font_1)
                .mb(px(12.0))
                .child(prompt.title.clone()),
        )
        .child(text_input(&prompt.input, palette, 32.0, 14.4, window, cx));

    if let Some(error) = &prompt.error {
        panel = panel.child(
            div()
                .mt(px(6.0))
                .text_size(px(11.2))
                .text_color(palette.red)
                .child(error.clone()),
        );
    }

    panel.child(
        div()
            .mt(px(12.0))
            .flex()
            .justify_end()
            .gap(px(8.0))
            .child(secondary_button(strings.cancel, palette).on_click(cx.listener(
                |this, _, _, cx| {
                    this.modal = None;
                    cx.notify();
                },
            )))
            .child(primary_button(strings.save, palette).on_click(cx.listener(
                |this, _, window, cx| this.save_prompt(window, cx),
            ))),
    )
}

fn confirm_panel(
    confirm: &ConfirmModal,
    strings: &Strings,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> Div {
    let mut panel = div()
        .w(px(420.0))
        .flex()
        .flex_col()
        .p(px(16.0))
        .bg(palette.panel_bg)
        .border_1()
        .border_color(palette.border)
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.font_1)
                .child(confirm.message.clone()),
        );

    if let Some(error) = &confirm.error {
        panel = panel.child(
            div()
                .mt(px(6.0))
                .text_size(px(11.2))
                .text_color(palette.red)
                .child(error.clone()),
        );
    }

    panel.child(
        div()
            .mt(px(16.0))
            .flex()
            .justify_end()
            .gap(px(8.0))
            .child(secondary_button(strings.cancel, palette).on_click(cx.listener(
                |this, _, _, cx| {
                    this.modal = None;
                    cx.notify();
                },
            )))
            .child(primary_button(strings.ok, palette).on_click(cx.listener(
                |this, _, _, cx| this.apply_confirm(cx),
            ))),
    )
}

fn menu_layer(menu: &MenuState, palette: Palette, cx: &mut Context<AppView>) -> AnyElement {
    let mut panel = div()
        .absolute()
        .left(px(menu.x))
        .top(px(menu.y))
        .min_w(px(148.0))
        .py(px(6.0))
        .bg(palette.panel_bg)
        .border_1()
        .border_color(palette.border)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

    for (index, item) in menu.items.iter().enumerate() {
        let action = item.action.clone();
        let color = if item.danger {
            palette.red
        } else {
            palette.font_1
        };
        panel = panel.child(
            div()
                .id(ElementId::named_usize("menu-item", index))
                .h(px(32.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .text_size(px(13.6))
                .text_color(color)
                .cursor_pointer()
                .hover(move |style| style.bg(palette.hover_bg).text_color(color))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.perform(action.clone(), window, cx);
                }))
                .child(item.label.clone()),
        );
    }

    div()
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .w_full()
        .h_full()
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
            this.menu = None;
            cx.notify();
        }))
        .child(panel)
        .into_any_element()
}

fn auth_choice(
    id: &'static str,
    label: &'static str,
    selected: bool,
    palette: Palette,
    cx: &mut Context<AppView>,
) -> Stateful<Div> {
    let background = if selected {
        palette.selected_bg
    } else {
        palette.panel_bg
    };
    let text_color = if selected {
        palette.accent
    } else {
        palette.font_2
    };
    let target = if id == "auth-key" {
        managi_core::AuthKind::PrivateKey
    } else {
        managi_core::AuthKind::Password
    };

    div()
        .id(id)
        .h(px(30.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .border_1()
        .border_color(if selected {
            palette.accent
        } else {
            palette.border
        })
        .bg(background)
        .text_size(px(12.8))
        .text_color(text_color)
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(Modal::Node(form)) = &mut this.modal {
                form.auth = target;
            }
            cx.notify();
        }))
        .child(label.to_string())
}

fn field(label: &'static str, input: impl IntoElement, palette: Palette) -> Div {    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .mb(px(10.0))
        .child(
            div()
                .text_size(px(12.8))
                .text_color(palette.font_2)
                .child(label.to_string()),
        )
        .child(input)
}

fn secondary_button(label: &'static str, palette: Palette) -> Stateful<Div> {
    div()
        .id(label)
        .h(px(30.0))
        .px(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(palette.border)
        .text_color(palette.font_1)
        .text_size(px(12.8))
        .cursor_pointer()
        .hover(move |style| {
            style
                .border_color(palette.accent)
                .text_color(palette.accent)
        })
        .child(label.to_string())
}

fn primary_button(label: &'static str, palette: Palette) -> Stateful<Div> {
    div()
        .id(label)
        .h(px(30.0))
        .px(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(palette.accent)
        .text_color(palette.bg)
        .text_size(px(12.8))
        .cursor_pointer()
        .child(label.to_string())
}

pub(crate) fn sidebar_width(collapsed: bool) -> f32 {
    if collapsed {
        SIDEBAR_COLLAPSED
    } else {
        SIDEBAR_WIDTH
    }
}
