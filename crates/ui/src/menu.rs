//! 右键菜单的数据模型。菜单在根层以绝对定位浮层绘制，避免各子树各自处理层级。

#[derive(Clone, Debug)]
pub(crate) struct MenuItem {
    pub label: String,
    pub action: MenuAction,
    pub danger: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum MenuAction {
    EditNode(String),
    DeleteNode(String),
    MoveNode { node: String, group: String },
    NewGroup,
    RenameGroup(String),
    DeleteGroup(String),
    CloseTab(String),
    CloseOthers(String),
    CloseAll,
}

pub(crate) struct MenuState {
    pub x: f32,
    pub y: f32,
    pub items: Vec<MenuItem>,
}
