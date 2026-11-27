//! 根视图：布局装配、右键菜单、弹窗，以及所有会改配置的动作。

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;
use managi_core::secret::{self, SecretKind};
use managi_core::ssh_config::{default_paths, remove_managed_node, upsert_managed_node};
use managi_core::{paths::home_dir, Config, Node, NodeSource};
use managi_ssh::{AuthMethod, ConnectRequest};
use managi_term::encode_key;

use crate::batch_runtime::BatchRuntime;
use crate::i18n::Strings;
use crate::menu::{MenuAction, MenuItem, MenuState};
use crate::modal::{ConfirmAction, ConfirmModal, Modal, NodeForm, PromptAction, PromptModal};
use crate::sftp_runtime::{SftpRuntime, join_path};
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
    pub(crate) batch_command: Entity<InputState>,
    pub(crate) modal: Option<Modal>,
    pub(crate) menu: Option<MenuState>,
    pub(crate) terminals: HashMap<String, TerminalRuntime>,
    pub(crate) sftp: HashMap<String, SftpRuntime>,
    pub(crate) batches: HashMap<String, BatchRuntime>,
    pub(crate) passwords: HashMap<String, String>,
    pub(crate) term_focus: FocusHandle,
    pub(crate) term_search: Entity<InputState>,
    pub(crate) terminal_search_open: bool,
    pub(crate) cursor_blink_at: Instant,
    pub(crate) quick_open: bool,
    last_sftp_click: Option<(String, String, Instant)>,
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

        let batch_command = cx.new(|cx| {
            InputState::new(window, cx).placeholder(strings.batch_command_placeholder)
        });

        let term_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(strings.terminal_search_placeholder)
        });

        cx.subscribe_in(&term_search, window, |this, state, event, _window, cx| {
            if matches!(event, InputEvent::Change) {
                let query = state.read(cx).value().to_string();
                this.update_terminal_search(&query, cx);
            }
        })
        .detach();

        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let alive = this
                .update(cx, |this, cx| {
                    if this
                        .state
                        .active_tab()
                        .is_some_and(|tab| tab.kind == TabKind::Terminal)
                    {
                        cx.notify();
                    }
                })
                .is_ok();
            if !alive {
                break;
            }
        })
        .detach();

        Self {
            state,
            search,
            batch_command,
            term_search,
            terminal_search_open: false,
            cursor_blink_at: Instant::now(),
            modal: None,
            menu: None,
            terminals: HashMap::new(),
            sftp: HashMap::new(),
            batches: HashMap::new(),
            passwords: HashMap::new(),
            term_focus: cx.focus_handle(),
            quick_open: false,
            last_sftp_click: None,
        }
    }

    /// 先查进程内缓存，再查系统钥匙串；命中后写回缓存，避免重复打开凭据库。
    fn credential(&mut self, node: &Node, kind: SecretKind) -> Option<String> {
        if let Some(value) = self.passwords.get(&node.id).cloned() {
            return Some(value);
        }
        let value = secret::get(&node.id, kind)?;
        self.passwords.insert(node.id.clone(), value.clone());
        Some(value)
    }

    pub(crate) fn export_config(&mut self, cx: &mut Context<Self>) {
        let dir = home_dir().unwrap_or_else(std::env::temp_dir);
        let receiver = cx.prompt_for_new_path(&dir, Some("managi-config.toml"));
        let config = self.state.config.clone();
        let strings = self.state.strings();

        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = receiver.await else {
                return;
            };
            let result = config.save(&path);
            this.update(cx, |this, cx| {
                this.state.notice = Some(match result {
                    Ok(()) => format!("{} {}", strings.export_done, path.display()),
                    Err(error) => format!("{}: {error}", strings.save_failed),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn import_config(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择 Managi 配置文件".into()),
        });
        let strings = self.state.strings();

        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.first() else {
                return;
            };
            let imported = match Config::load(path) {
                Ok(config) => config,
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.state.notice = Some(format!("{}: {error}", strings.import_failed));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            this.update(cx, |this, cx| {
                let config_path = this.state.config_path.clone();
                let mut next = this.state.config.clone();
                let added = next.merge_from(&imported);
                match next.save(&config_path) {
                    Ok(()) => {
                        this.state.config = next;
                        this.state.notice = Some(format!("{} {}", strings.import_done, added));
                    }
                    Err(error) => {
                        this.state.notice = Some(format!("{}: {error}", strings.save_failed));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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

        if node.uses_key() {
            let passphrase = self.credential(node, SecretKind::Passphrase);
            self.start_session(&tab_id, node, passphrase, window, cx);
        } else if let Some(password) = self.credential(node, SecretKind::Password) {
            self.start_session(&tab_id, node, Some(password), window, cx);
        } else {
            let strings = self.state.strings();
            self.open_prompt(
                format!("{} {}", strings.connect_prefix, node.name),
                None,
                PromptAction::ConnectNode {
                    node_id: node.id.clone(),
                    kind: TabKind::Terminal,
                },
                window,
                cx,
            );
        }
    }

    pub(crate) fn open_sftp(
        &mut self,
        node: &Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.open_node_tab(TabKind::Sftp, node);
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        if self.sftp.contains_key(&tab_id) {
            cx.notify();
            return;
        }

        if node.uses_key() {
            let passphrase = self.credential(node, SecretKind::Passphrase);
            self.start_sftp(&tab_id, node, passphrase, window, cx);
        } else if let Some(password) = self.credential(node, SecretKind::Password) {
            self.start_sftp(&tab_id, node, Some(password), window, cx);
        } else {
            let strings = self.state.strings();
            self.open_prompt(
                format!("{} {}", strings.connect_prefix, node.name),
                None,
                PromptAction::ConnectNode {
                    node_id: node.id.clone(),
                    kind: TabKind::Sftp,
                },
                window,
                cx,
            );
        }
    }

    fn connect_request(
        &self,
        node: &Node,
        password: Option<String>,
    ) -> Option<ConnectRequest> {
        let auth = match node.auth {
            managi_core::AuthKind::Password => {
                let password = password.or_else(|| self.passwords.get(&node.id).cloned())?;
                AuthMethod::Password(password)
            }
            managi_core::AuthKind::PrivateKey => {
                let path = node.key_path.trim();
                if path.is_empty() {
                    return None;
                }
                AuthMethod::PrivateKey {
                    path: PathBuf::from(path),
                    passphrase: password
                        .or_else(|| self.passwords.get(&node.id).cloned())
                        .or_else(|| secret::get(&node.id, SecretKind::Passphrase)),
                }
            }
            managi_core::AuthKind::Auto => {
                if node.uses_key() {
                    AuthMethod::PrivateKey {
                        path: PathBuf::from(node.key_path.trim()),
                        passphrase: password
                            .or_else(|| self.passwords.get(&node.id).cloned())
                            .or_else(|| secret::get(&node.id, SecretKind::Passphrase)),
                    }
                } else {
                    let password = password
                        .or_else(|| self.passwords.get(&node.id).cloned())
                        .or_else(|| secret::get(&node.id, SecretKind::Password))?;
                    AuthMethod::Password(password)
                }
            }
        };

        let known_hosts_path = home_dir()
            .map(|home| home.join(".ssh").join("known_hosts"))
            .filter(|path| path.parent().is_some_and(|dir| dir.exists()))
            .unwrap_or_else(|| {
                self.state
                    .config_path
                    .parent()
                    .map(|parent| parent.join("known_hosts"))
                    .unwrap_or_else(|| PathBuf::from("known_hosts"))
            });

        Some(ConnectRequest {
            host: node.host.clone(),
            port: node.port,
            username: node.username.clone(),
            auth,
            known_hosts_path,
            columns: 80,
            rows: 24,
        })
    }

    fn start_session(
        &mut self,
        tab_id: &str,
        node: &Node,
        password: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self.connect_request(node, password) else {
            self.state.notice = Some(self.state.strings().key_path_required.to_string());
            cx.notify();
            return;
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

    fn start_sftp(
        &mut self,
        tab_id: &str,
        node: &Node,
        password: Option<String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self.connect_request(node, password) else {
            self.state.notice = Some(self.state.strings().key_path_required.to_string());
            cx.notify();
            return;
        };

        match managi_ssh::sftp::spawn(request) {
            Ok(handle) => {
                self.sftp
                    .insert(tab_id.to_string(), SftpRuntime::new(handle));
            }
            Err(error) => {
                self.state.notice = Some(error.to_string());
            }
        }
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

        if node.uses_key() {
            self.start_session(&tab_id, &node, None, window, cx);
        } else if let Some(password) = self.passwords.get(&node.id).cloned() {
            self.start_session(&tab_id, &node, Some(password), window, cx);
        } else {
            let strings = self.state.strings();
            self.open_prompt(
                format!("{} {}", strings.connect_prefix, node.name),
                None,
                PromptAction::ConnectNode {
                    node_id: node.id.clone(),
                    kind: TabKind::Terminal,
                },
                window,
                cx,
            );
        }
    }

    pub(crate) fn reconnect_sftp_active(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        self.sftp.remove(&tab_id);

        if node.uses_key() {
            self.start_sftp(&tab_id, &node, None, window, cx);
        } else if let Some(password) = self.passwords.get(&node.id).cloned() {
            self.start_sftp(&tab_id, &node, Some(password), window, cx);
        } else {
            let strings = self.state.strings();
            self.open_prompt(
                format!("{} {}", strings.connect_prefix, node.name),
                None,
                PromptAction::ConnectNode {
                    node_id: node.id.clone(),
                    kind: TabKind::Sftp,
                },
                window,
                cx,
            );
        }
    }

    pub(crate) fn cursor_blink_on(&self) -> bool {
        let elapsed = self.cursor_blink_at.elapsed().as_millis();
        elapsed < 500 || (elapsed / 500) % 2 == 0
    }

    pub(crate) fn handle_terminal_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let control = keystroke.modifiers.control;
        let shift = keystroke.modifiers.shift;
        self.cursor_blink_at = Instant::now();

        if control && key.eq_ignore_ascii_case("f") && !shift {
            self.open_terminal_search(window, cx);
            return;
        }
        if control && shift && key.eq_ignore_ascii_case("k") {
            self.toggle_quick(cx);
            return;
        }
        if control && shift && key.eq_ignore_ascii_case("c") {
            let text = self
                .terminals
                .get(&tab_id)
                .map(|runtime| {
                    runtime
                        .model
                        .selection_text()
                        .filter(|text| !text.is_empty())
                        .unwrap_or_else(|| runtime.model.screen_text())
                })
                .unwrap_or_default();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            return;
        }
        if control && shift && key.eq_ignore_ascii_case("v") {
            let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
                return;
            };
            self.paste_terminal_text(&text, cx);
            return;
        }
        if control && shift && key.eq_ignore_ascii_case("a") {
            if let Some(runtime) = self.terminals.get_mut(&tab_id) {
                runtime.model.select_all();
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
        // Tab 直接写进终端并截断事件，避免被 GPUI 的焦点遍历吃掉。
        if !control && key.eq_ignore_ascii_case("tab") {
            if let Some(runtime) = self.terminals.get_mut(&tab_id) {
                if runtime.is_live() {
                    runtime
                        .handle
                        .write(if shift { b"\x1b[Z".to_vec() } else { b"\t".to_vec() });
                }
            }
            cx.stop_propagation();
            cx.notify();
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

    fn paste_terminal_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.cursor_blink_at = Instant::now();
        let Some(tab_id) = self.state.active_tab.clone() else {
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
    }

    /// 鼠标左键：Ctrl 点链接优先；否则开始选区。
    pub(crate) fn terminal_mouse_down(
        &mut self,
        line: usize,
        column: usize,
        clicks: usize,
        control: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.term_focus.focus(window, cx);
        self.cursor_blink_at = Instant::now();
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let Some(runtime) = self.terminals.get_mut(&tab_id) else {
            return;
        };
        if control {
            if let Some(url) = runtime.model.url_at(line, column) {
                cx.open_url(&url);
                return;
            }
        }
        runtime.model.begin_selection_at(line, column, clicks);
        cx.notify();
    }

    pub(crate) fn terminal_mouse_move(
        &mut self,
        line: usize,
        column: usize,
        tail: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let Some(runtime) = self.terminals.get_mut(&tab_id) else {
            return;
        };
        if runtime.model.has_selection() {
            runtime.model.update_selection(line, column, tail);
            cx.notify();
        }
    }

    pub(crate) fn terminal_context_menu(
        &mut self,
        x: f32,
        y: f32,
        line: usize,
        column: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.state.strings();
        let has_link = self
            .terminals
            .get(&self.state.active_tab.clone().unwrap_or_default())
            .and_then(|runtime| runtime.model.url_at(line, column))
            .is_some();
        let mut items = vec![
            MenuItem {
                label: strings.copy.to_string(),
                action: MenuAction::CopySelection,
                danger: false,
            },
            MenuItem {
                label: strings.paste.to_string(),
                action: MenuAction::PasteClipboard,
                danger: false,
            },
            MenuItem {
                label: strings.select_all.to_string(),
                action: MenuAction::SelectAll,
                danger: false,
            },
            MenuItem {
                label: strings.terminal_search.to_string(),
                action: MenuAction::SearchTerminal,
                danger: false,
            },
            MenuItem {
                label: strings.clear_screen.to_string(),
                action: MenuAction::ClearScreen,
                danger: false,
            },
        ];
        if has_link {
            items.insert(
                0,
                MenuItem {
                    label: strings.open_link.to_string(),
                    action: MenuAction::OpenLink(line, column),
                    danger: false,
                },
            );
        }
        self.open_menu(x, y, items, window, cx);
        cx.stop_propagation();
    }

    pub(crate) fn open_terminal_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_search_open = true;
        let handle = self.term_search.read(cx).focus_handle(cx);
        handle.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn close_terminal_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_search_open = false;
        if let Some(tab_id) = self.state.active_tab.clone() {
            if let Some(runtime) = self.terminals.get_mut(&tab_id) {
                runtime.model.clear_search();
            }
        }
        self.term_focus.focus(window, cx);
        cx.notify();
    }

    fn update_terminal_search(&mut self, query: &str, cx: &mut Context<Self>) {
        if !self.terminal_search_open {
            return;
        }
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        if let Some(runtime) = self.terminals.get_mut(&tab_id) {
            runtime.model.set_search(query);
            cx.notify();
        }
    }

    pub(crate) fn terminal_search_step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        if let Some(runtime) = self.terminals.get_mut(&tab_id) {
            runtime.model.search_next(forward);
            cx.notify();
        }
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

    pub(crate) fn toggle_quick(&mut self, cx: &mut Context<Self>) {
        self.quick_open = !self.quick_open;
        cx.notify();
    }

    pub(crate) fn run_quick_command(
        &mut self,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let Some(runtime) = self.terminals.get_mut(&tab_id) else {
            return;
        };
        if !runtime.is_live() {
            return;
        }
        let mut payload = command.as_bytes().to_vec();
        payload.push(b'\r');
        runtime.handle.write(payload);
        self.term_focus.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn sftp_select(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let Some(runtime) = self.sftp.get_mut(&tab_id) else {
            return;
        };
        let Some(entry) = runtime.entries.iter().find(|entry| entry.name == name) else {
            return;
        };
        let is_dir = entry.is_dir;
        let hit = self.last_sftp_click.as_ref().is_some_and(|(tab, last, at)| {
            tab == &tab_id && last == name && at.elapsed() < Duration::from_millis(400)
        });
        if hit && is_dir {
            runtime.open_dir(name);
            self.last_sftp_click = None;
        } else {
            runtime.selected = Some(name.to_string());
            self.last_sftp_click = Some((tab_id, name.to_string(), Instant::now()));
        }
        cx.notify();
    }

    pub(crate) fn sftp_up(&mut self, cx: &mut Context<Self>) {
        if let Some(runtime) = self.active_sftp_mut() {
            runtime.up();
            cx.notify();
        }
    }

    pub(crate) fn sftp_refresh(&mut self, cx: &mut Context<Self>) {
        if let Some(runtime) = self.active_sftp_mut() {
            runtime.refresh();
            cx.notify();
        }
    }

    pub(crate) fn sftp_new_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        if !self.sftp.contains_key(&tab_id) {
            return;
        }
        let strings = self.state.strings();
        self.open_prompt(
            strings.sftp_new_dir.to_string(),
            None,
            PromptAction::SftpMkdir(tab_id),
            window,
            cx,
        );
    }

    pub(crate) fn sftp_upload(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        if !self.sftp.contains_key(&tab_id) {
            return;
        }
        let strings = self.state.strings();
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(strings.sftp_upload_title.into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            this.update(cx, |this, cx| {
                let Some(runtime) = this.sftp.get_mut(&tab_id) else {
                    return;
                };
                for path in paths {
                    runtime.upload(path);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn sftp_drop_upload(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let Some(runtime) = self.sftp.get_mut(&tab_id) else {
            return;
        };
        let mut sent = 0usize;
        for path in paths {
            if path.is_file() {
                runtime.upload(path.clone());
                sent += 1;
            }
        }
        if sent == 0 {
            runtime.error = Some("只支持拖入文件，暂不支持目录".into());
        }
        cx.notify();
    }

    pub(crate) fn sftp_download(&mut self, cx: &mut Context<Self>) {
        let strings = self.state.strings();
        let selection = self.active_sftp_mut().and_then(|runtime| {
            let name = runtime.selected.clone()?;
            let is_file = runtime
                .entries
                .iter()
                .any(|entry| entry.name == name && !entry.is_dir);
            Some((name, is_file))
        });
        let Some((name, is_file)) = selection else {
            self.state.notice = Some(strings.sftp_no_selection.to_string());
            cx.notify();
            return;
        };
        if !is_file {
            self.state.notice = Some(strings.sftp_no_selection.to_string());
            cx.notify();
            return;
        }
        if let Some(runtime) = self.active_sftp_mut() {
            runtime.download(&name);
        }
        cx.notify();
    }

    pub(crate) fn sftp_delete(&mut self, cx: &mut Context<Self>) {
        let strings = self.state.strings();
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let selection = self.sftp.get(&tab_id).and_then(|runtime| {
            let name = runtime.selected.clone()?;
            runtime
                .entries
                .iter()
                .find(|entry| entry.name == name)
                .map(|entry| (entry.name.clone(), entry.is_dir))
        });
        let Some((name, is_dir)) = selection else {
            self.state.notice = Some(strings.sftp_no_selection.to_string());
            cx.notify();
            return;
        };
        let message = format!("{} {}?", strings.sftp_delete_confirm, name);
        self.open_confirm(
            message,
            ConfirmAction::SftpDelete {
                tab_id,
                name,
                is_dir,
            },
            cx,
        );
    }

    fn active_sftp_mut(&mut self) -> Option<&mut SftpRuntime> {
        let tab_id = self.state.active_tab.clone()?;
        self.sftp.get_mut(&tab_id)
    }

    pub(crate) fn open_batch(&mut self, cx: &mut Context<Self>) {
        self.state.open_singleton(TabKind::Batch);
        cx.notify();
    }

    pub(crate) fn run_batch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.state.strings();
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        let command = self.batch_command.read(cx).value().trim().to_string();
        if command.is_empty() {
            self.state.notice = Some(strings.batch_command_placeholder.to_string());
            cx.notify();
            return;
        }

        let selected = self.state.selected_nodes.clone();
        if selected.is_empty() {
            self.state.notice = Some(strings.batch_no_selection.to_string());
            cx.notify();
            return;
        }

        let nodes: Vec<Node> = selected
            .iter()
            .filter_map(|id| {
                self.state
                    .config
                    .nodes
                    .iter()
                    .find(|node| &node.id == id)
                    .cloned()
            })
            .collect();
        if nodes.is_empty() {
            self.state.notice = Some(strings.batch_no_selection.to_string());
            cx.notify();
            return;
        }

        let mut runtime = BatchRuntime::new(command.clone());
        for node in nodes {
            let Some(request) = self.connect_request(&node, None) else {
                runtime.push_error(
                    node.name.clone(),
                    node.id.clone(),
                    strings.batch_no_password.to_string(),
                );
                continue;
            };
            match managi_ssh::exec::spawn(request, command.clone()) {
                Ok(handle) => runtime.push(node.name.clone(), node.id.clone(), Some(handle)),
                Err(error) => {
                    runtime.push_error(node.name.clone(), node.id.clone(), error.to_string())
                }
            }
        }

        self.batches.insert(tab_id, runtime);
        self.batch_command.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub(crate) fn batch_stop(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(tab_id) = self.state.active_tab.clone() else {
            return;
        };
        if let Some(runtime) = self.batches.get_mut(&tab_id) {
            runtime.stop(index);
            cx.notify();
        }
    }

    pub(crate) fn batch_stop_all(&mut self, cx: &mut Context<Self>) {
        if let Some(runtime) = self.active_batch_mut() {
            runtime.stop_all();
            cx.notify();
        }
    }

    fn active_batch_mut(&mut self) -> Option<&mut BatchRuntime> {
        let tab_id = self.state.active_tab.clone()?;
        self.batches.get_mut(&tab_id)
    }

    fn poll_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active_tab = self.state.active_tab.clone();
        let active_kind = self.state.active_tab().map(|tab| tab.kind);

        let valid_tabs: Vec<String> = self
            .state
            .tabs
            .iter()
            .map(|tab| tab.id.clone())
            .collect();
        self.terminals
            .retain(|tab_id, _| valid_tabs.iter().any(|id| id == tab_id));
        self.sftp
            .retain(|tab_id, _| valid_tabs.iter().any(|id| id == tab_id));
        self.batches
            .retain(|tab_id, _| valid_tabs.iter().any(|id| id == tab_id));

        let font_size = f32::from(self.state.config.settings.terminal_font_size);
        let quick_width = if self.quick_open { 156.0 } else { 0.0 };
        let width = window.bounds().size.width.as_f32()
            - sidebar_width(self.state.sidebar_collapsed)
            - quick_width
            - 16.0;
        let height = window.bounds().size.height.as_f32() - BAR_HEIGHT * 2.0 - 16.0;
        let (cell_width, line_height) = crate::terminal_view::mono_metrics(window, font_size);
        let columns = ((width / cell_width).floor() as u16).max(20);
        let rows = ((height / line_height).floor() as u16).max(4);

        let mut animate = false;
        for (tab_id, runtime) in &mut self.terminals {
            let changed = runtime.poll();
            if active_kind == Some(TabKind::Terminal) && active_tab.as_deref() == Some(tab_id.as_str()) {
                runtime.resize(columns, rows);
            }
            if runtime.is_live() {
                animate = true;
            }
            if changed
                && active_kind == Some(TabKind::Terminal)
                && active_tab.as_deref() == Some(tab_id.as_str())
            {
                cx.notify();
            }
        }

        for (tab_id, runtime) in &mut self.sftp {
            let changed = runtime.poll();
            // 握手完成前连接态还是 false；只要还在加载就必须继续出帧，否则失败事件
            // 会因为没有下一帧而永远读不到。
            if runtime.loading || runtime.progress.is_some() || runtime.is_live() {
                animate = true;
            }
            if changed && active_kind == Some(TabKind::Sftp) && active_tab.as_deref() == Some(tab_id.as_str()) {
                cx.notify();
            }
        }

        for (tab_id, runtime) in &mut self.batches {
            let changed = runtime.poll();
            if runtime.running() {
                animate = true;
            }
            if changed && active_kind == Some(TabKind::Batch) && active_tab.as_deref() == Some(tab_id.as_str()) {
                cx.notify();
            }
        }

        // 仍有活动会话/传输/批量任务时持续请求下一帧。
        if animate {
            window.request_animation_frame();
        }
    }

    pub(crate) fn close_tab(&mut self, id: &str, cx: &mut Context<Self>) {
        self.terminals.remove(id);
        self.sftp.remove(id);
        self.batches.remove(id);
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
        let node = node_id.and_then(|id| self.state.find_node(id));
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
        let masked = matches!(&action, PromptAction::ConnectNode { .. });
        let strings = self.state.strings();
        let placeholder = match &action {
            PromptAction::ConnectNode { .. } => strings.password_placeholder,
            PromptAction::SftpMkdir(_) => strings.sftp_dir_placeholder,
            PromptAction::NewGroup => strings.new_group_placeholder,
            PromptAction::RenameGroup(_) => strings.rename_group_placeholder,
        };
        let prompt = PromptModal::create(title, initial, action, placeholder, masked, window, cx);
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
        let source = form.source;
        let alias_text = form.alias.clone();
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
        node.source = source;

        if source == NodeSource::Managed {
            let alias = if alias_text.trim().is_empty() {
                node.name.split_whitespace().collect::<Vec<_>>().join("-")
            } else {
                alias_text.trim().to_string()
            };
            if alias.is_empty()
                || alias
                    .chars()
                    .any(|ch| ch.is_whitespace() || matches!(ch, '#' | '!' | '*' | '?'))
            {
                self.set_modal_error(strings.invalid_alias.to_string());
                cx.notify();
                return;
            }
            if self
                .state
                .system_nodes
                .iter()
                .any(|existing| existing.id != node_id && existing.alias.eq_ignore_ascii_case(&alias))
            {
                self.set_modal_error(strings.invalid_alias.to_string());
                cx.notify();
                return;
            }

            node.alias = alias;
            node.group = managi_core::ssh_config::SYSTEM_GROUP.to_string();
            let result = default_paths()
                .map_err(|error| error.to_string())
                .and_then(|paths| {
                    upsert_managed_node(&paths, &node).map_err(|error| error.to_string())
                });
            match result {
                Ok(()) => {
                    self.state.rescan_system_nodes();
                    self.state.notice = Some(strings.ssh_managed_note.to_string());
                    self.modal = None;
                }
                Err(error) => self.set_modal_error(format!("{}: {error}", strings.ssh_config_failed)),
            }
            cx.notify();
            return;
        }

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
                PromptAction::ConnectNode { .. } => strings.password_required,
                _ => strings.name_required,
            };
            self.set_modal_error(message.to_string());
            cx.notify();
            return;
        }

        match action {
            PromptAction::ConnectNode { node_id, kind } => {
                let node = self.state.find_node(&node_id);
                let secret_kind = match node.as_ref() {
                    Some(node) if node.uses_key() => SecretKind::Passphrase,
                    _ => SecretKind::Password,
                };
                if let Err(error) = secret::set(&node_id, secret_kind, &value) {
                    self.state.notice = Some(format!("{}: {error}", strings.save_failed));
                }
                self.passwords.insert(node_id.clone(), value);
                self.modal = None;
                if let Some(node) = node {
                    let Some(tab_id) = self.state.active_tab.clone() else {
                        return;
                    };
                    let password = self.passwords.get(&node_id).cloned();
                    if kind == TabKind::Sftp {
                        self.start_sftp(&tab_id, &node, password, window, cx);
                    } else {
                        self.start_session(&tab_id, &node, password, window, cx);
                    }
                }
                cx.notify();
                return;
            }
            PromptAction::SftpMkdir(tab_id) => {
                self.modal = None;
                if let Some(runtime) = self.sftp.get_mut(&tab_id) {
                    runtime.mkdir(&value);
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
                let node = self.state.find_node(&id);
                match node.map(|node| node.source) {
                    Some(NodeSource::Managed) => {
                        if let Some(node) = self.state.find_node(&id) {
                            secret::forget_node(&node);
                        }
                        let result = default_paths()
                            .map_err(|error| error.to_string())
                            .and_then(|paths| {
                                remove_managed_node(&paths, &id).map_err(|error| error.to_string())
                            });
                        match result {
                            Ok(()) => {
                                self.state.selected_nodes.retain(|node| node != &id);
                                self.state.rescan_system_nodes();
                                self.modal = None;
                                cx.notify();
                            }
                            Err(error) => {
                                self.set_modal_error(format!(
                                    "{}: {error}",
                                    strings.ssh_config_failed
                                ));
                                cx.notify();
                            }
                        }
                        return;
                    }
                    Some(NodeSource::SshConfig) => {
                        self.state.notice = Some(strings.ssh_readonly.to_string());
                        self.modal = None;
                        cx.notify();
                        return;
                    }
                    _ => {}
                }
                if let Some(node) = self.state.find_node(&id) {
                    secret::forget_node(&node);
                }
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
            ConfirmAction::SftpDelete {
                tab_id,
                name,
                is_dir,
            } => {
                self.modal = None;
                if let Some(runtime) = self.sftp.get_mut(&tab_id) {
                    let path = join_path(&runtime.path, &name);
                    runtime.handle.send(managi_ssh::SftpCommand::Delete { path, is_dir });
                    runtime.selected = None;
                }
                cx.notify();
                return;
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
                    .find_node(&id)
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
                self.sftp
                    .retain(|tab_id, _| keep_id.as_deref() == Some(tab_id.as_str()));
                self.batches
                    .retain(|tab_id, _| keep_id.as_deref() == Some(tab_id.as_str()));
                self.state.close_others(&id);
                cx.notify();
            }
            MenuAction::CloseAll => {
                self.terminals.clear();
                self.sftp.clear();
                self.batches.clear();
                self.state.close_all();
                cx.notify();
            }
            MenuAction::CopySelection => {
                if let Some(tab_id) = self.state.active_tab.clone() {
                    if let Some(runtime) = self.terminals.get(&tab_id) {
                        let text = runtime
                            .model
                            .selection_text()
                            .filter(|text| !text.is_empty())
                            .unwrap_or_else(|| runtime.model.screen_text());
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
            }
            MenuAction::PasteClipboard => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.paste_terminal_text(&text, cx);
                }
            }
            MenuAction::SelectAll => {
                if let Some(tab_id) = self.state.active_tab.clone() {
                    if let Some(runtime) = self.terminals.get_mut(&tab_id) {
                        runtime.model.select_all();
                    }
                }
                cx.notify();
            }
            MenuAction::SearchTerminal => self.open_terminal_search(window, cx),
            MenuAction::ClearScreen => {
                if let Some(tab_id) = self.state.active_tab.clone() {
                    if let Some(runtime) = self.terminals.get_mut(&tab_id) {
                        if runtime.is_live() {
                            runtime.handle.write(b"\x0c".to_vec());
                        }
                    }
                }
                cx.notify();
            }
            MenuAction::OpenLink(line, column) => {
                if let Some(tab_id) = self.state.active_tab.clone() {
                    if let Some(url) = self
                        .terminals
                        .get(&tab_id)
                        .and_then(|runtime| runtime.model.url_at(line, column))
                    {
                        cx.open_url(&url);
                    }
                }
            }
        }
    }

    pub(crate) fn node_menu(&self, node_id: &str) -> Vec<MenuItem> {
        let strings = self.state.strings();
        let Some(node) = self.state.find_node(node_id) else {
            return Vec::new();
        };
        if node.source == NodeSource::SshConfig {
            return Vec::new();
        }

        let mut items = vec![MenuItem {
            label: strings.edit.to_string(),
            action: MenuAction::EditNode(node_id.to_string()),
            danger: false,
        }];

        if node.source == NodeSource::Local {
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
        self.poll_sessions(window, cx);
        let palette = theme::palette(self.state.config.settings.theme);
        let strings = self.state.strings();

        let sidebar = crate::sidebar::render(&self.state, &self.search, &strings, palette, window, cx);
        let tabs = crate::tab_bar::render(&self.state, &strings, palette, cx);
        let sidebar_width = if self.state.sidebar_collapsed {
            SIDEBAR_COLLAPSED
        } else {
            SIDEBAR_WIDTH
        };
        let workspace = crate::workspace::render(
            &self.state,
            &strings,
            palette,
            &self.terminals,
            &self.sftp,
            &self.batches,
            &self.batch_command,
            &self.term_focus,
            &self.term_search,
            self.terminal_search_open,
            self.cursor_blink_on(),
            self.quick_open,
            sidebar_width,
            window,
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
                    "auth-auto",
                    strings.auth_auto,
                    form.auth == managi_core::AuthKind::Auto,
                    palette,
                    cx,
                ))
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

    if form.auth != managi_core::AuthKind::Password {
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
    let target = match id {
        "auth-key" => managi_core::AuthKind::PrivateKey,
        "auth-auto" => managi_core::AuthKind::Auto,
        _ => managi_core::AuthKind::Password,
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

pub(crate) fn primary_button(label: &'static str, palette: Palette) -> Stateful<Div> {
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
