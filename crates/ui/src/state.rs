//! 应用状态：配置、标签页、侧栏选择与搜索。这里不放 GPUI 实体，
//! 输入框实体由 `AppView` 持有，保持状态层可被独立测试。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use managi_core::{Config, Node};

use crate::i18n::Strings;

static NODE_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabKind {
    Terminal,
    Sftp,
    Settings,
    Batch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabStatus {
    Idle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsSection {
    Appearance,
    Terminal,
    Data,
    About,
}

#[derive(Clone, Debug)]
pub(crate) struct Tab {
    pub id: String,
    pub kind: TabKind,
    pub node: Option<Node>,
}

pub(crate) struct AppState {
    pub config: Config,
    pub config_path: PathBuf,
    pub version: String,
    pub tabs: Vec<Tab>,
    pub active_tab: Option<String>,
    pub next_tab: u64,
    pub selected_nodes: Vec<String>,
    pub collapsed_groups: Vec<String>,
    pub sidebar_collapsed: bool,
    pub query: String,
    pub notice: Option<String>,
    pub settings_section: SettingsSection,
}

pub(crate) struct DisplayGroup {
    pub label: String,
    pub all_hosts: bool,
    pub count: usize,
    pub nodes: Vec<Node>,
}

impl AppState {
    pub(crate) fn new(config: Config, config_path: PathBuf, version: String) -> Self {
        Self {
            config,
            config_path,
            version,
            tabs: Vec::new(),
            active_tab: None,
            next_tab: 0,
            selected_nodes: Vec::new(),
            collapsed_groups: Vec::new(),
            sidebar_collapsed: false,
            query: String::new(),
            notice: None,
            settings_section: SettingsSection::Appearance,
        }
    }

    pub(crate) fn strings(&self) -> Strings {
        Strings::for_lang(self.config.settings.language)
    }

    pub(crate) fn persist(&mut self) -> Result<(), String> {
        self.config
            .validate()
            .and_then(|_| self.config.save(&self.config_path))
            .map_err(|error| error.to_string())
    }

    pub(crate) fn tab_title(&self, tab: &Tab, strings: &Strings) -> String {
        match tab.kind {
            TabKind::Terminal => tab
                .node
                .as_ref()
                .map(|node| node.name.clone())
                .unwrap_or_else(|| strings.idle.to_string()),
            TabKind::Sftp => tab
                .node
                .as_ref()
                .map(|node| format!("{} {}", node.name, strings.tab_sftp_suffix))
                .unwrap_or_else(|| strings.tab_sftp_suffix.to_string()),
            TabKind::Settings => strings.tab_settings.to_string(),
            TabKind::Batch => strings.tab_batch.to_string(),
        }
    }

    pub(crate) fn open_node_tab(&mut self, kind: TabKind, node: &Node) {
        if let Some(existing) = self
            .tabs
            .iter()
            .find(|tab| tab.kind == kind && tab.node.as_ref().is_some_and(|n| n.id == node.id))
        {
            self.active_tab = Some(existing.id.clone());
            return;
        }

        let id = self.make_tab_id();
        self.tabs.push(Tab {
            id: id.clone(),
            kind,
            node: Some(node.clone()),
        });
        self.active_tab = Some(id);
    }

    pub(crate) fn open_singleton(&mut self, kind: TabKind) {
        if let Some(existing) = self.tabs.iter().find(|tab| tab.kind == kind) {
            self.active_tab = Some(existing.id.clone());
            return;
        }

        let id = self.make_tab_id();
        self.tabs.push(Tab {
            id: id.clone(),
            kind,
            node: None,
        });
        self.active_tab = Some(id);
    }

    fn make_tab_id(&mut self) -> String {
        self.next_tab += 1;
        format!("tab-{}", self.next_tab)
    }

    pub(crate) fn close_tab(&mut self, id: &str) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        self.tabs.remove(index);
        if self.active_tab.as_deref() == Some(id) {
            self.active_tab = self
                .tabs
                .get(index.saturating_sub(1))
                .or_else(|| self.tabs.get(index))
                .map(|tab| tab.id.clone());
        }
    }

    pub(crate) fn close_others(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter().find(|tab| tab.id == id).cloned() {
            self.tabs = vec![tab];
            self.active_tab = Some(id.to_string());
        }
    }

    pub(crate) fn close_all(&mut self) {
        self.tabs.clear();
        self.active_tab = None;
    }

    pub(crate) fn active_tab(&self) -> Option<&Tab> {
        let id = self.active_tab.as_deref()?;
        self.tabs.iter().find(|tab| tab.id == id)
    }

    pub(crate) fn is_selected(&self, node_id: &str) -> bool {
        self.selected_nodes.iter().any(|id| id == node_id)
    }

    pub(crate) fn toggle_node(&mut self, node_id: &str) {
        if let Some(index) = self.selected_nodes.iter().position(|id| id == node_id) {
            self.selected_nodes.remove(index);
        } else {
            self.selected_nodes.push(node_id.to_string());
        }
    }

    pub(crate) fn toggle_group(&mut self, node_ids: &[String]) {
        let all_selected = !node_ids.is_empty()
            && node_ids
                .iter()
                .all(|id| self.selected_nodes.iter().any(|selected| selected == id));

        if all_selected {
            self.selected_nodes
                .retain(|id| !node_ids.iter().any(|node| node == id));
        } else {
            for id in node_ids {
                if !self.selected_nodes.iter().any(|selected| selected == id) {
                    self.selected_nodes.push(id.clone());
                }
            }
        }
    }

    pub(crate) fn is_group_selected(&self, node_ids: &[String]) -> bool {
        !node_ids.is_empty()
            && node_ids
                .iter()
                .all(|id| self.selected_nodes.iter().any(|selected| selected == id))
    }

    pub(crate) fn is_group_partial(&self, node_ids: &[String]) -> bool {
        node_ids
            .iter()
            .any(|id| self.selected_nodes.iter().any(|selected| selected == id))
            && !self.is_group_selected(node_ids)
    }

    pub(crate) fn is_group_collapsed(&self, group: &str) -> bool {
        self.collapsed_groups.iter().any(|name| name == group)
    }

    pub(crate) fn toggle_collapsed(&mut self, group: &str) {
        if let Some(index) = self.collapsed_groups.iter().position(|name| name == group) {
            self.collapsed_groups.remove(index);
        } else {
            self.collapsed_groups.push(group.to_string());
        }
    }

    pub(crate) fn display_groups(&self, strings: &Strings) -> Vec<DisplayGroup> {
        let query = self.query.trim().to_lowercase();
        let mut all_nodes: Vec<&Node> = self.config.nodes.iter().collect();
        all_nodes.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.host.cmp(&b.host))
        });

        let matches = |node: &Node| {
            query.is_empty()
                || node.name.to_lowercase().contains(&query)
                || node.host.to_lowercase().contains(&query)
        };

        let mut groups = Vec::with_capacity(self.config.groups.len() + 1);
        let all_host_matches: Vec<Node> = all_nodes
            .iter()
            .filter(|node| matches(node))
            .map(|node| (**node).clone())
            .collect();
        groups.push(DisplayGroup {
            label: strings.all_hosts.to_string(),
            all_hosts: true,
            count: all_host_matches.len(),
            nodes: all_host_matches,
        });

        for group in &self.config.groups {
            let mut nodes: Vec<&Node> = all_nodes
                .iter()
                .copied()
                .filter(|node| node.group.as_str() == group.as_str())
                .collect();
            nodes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            let filtered: Vec<Node> = nodes
                .into_iter()
                .filter(|node| matches(node))
                .cloned()
                .collect();

            if !query.is_empty()
                && filtered.is_empty()
                && !group.to_lowercase().contains(&query)
            {
                continue;
            }

            groups.push(DisplayGroup {
                label: group.clone(),
                all_hosts: false,
                count: filtered.len(),
                nodes: filtered,
            });
        }

        groups
    }

    pub(crate) fn node_ids(&self, group: &str) -> Vec<String> {
        if group.is_empty() {
            self.config.nodes.iter().map(|node| node.id.clone()).collect()
        } else {
            self.config
                .nodes
                .iter()
                .filter(|node| node.group == group)
                .map(|node| node.id.clone())
                .collect()
        }
    }
}

/// 新节点 id：时间戳 + 进程内序号，避免引入 uuid 依赖。
pub(crate) fn new_node_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let seq = NODE_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("n{now:x}{seq:x}")
}
