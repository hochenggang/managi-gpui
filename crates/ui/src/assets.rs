//! 资源加载：自有图标优先，其余交给 gpui-component 的 Lucide 图标包。

use std::borrow::Cow;

use gpui_kit::*;

pub(crate) mod icons {
    pub(crate) const TERMINAL: &str = "icons/terminal.svg";
    pub(crate) const FOLDER: &str = "icons/folder.svg";
    pub(crate) const SETTING: &str = "icons/setting.svg";
    pub(crate) const BATCH: &str = "icons/batch.svg";
    pub(crate) const PLUS: &str = "icons/plus.svg";
    pub(crate) const CHEVRON_RIGHT: &str = "icons/chevron-right.svg";
    pub(crate) const CHEVRON_DOWN: &str = "icons/chevron-down.svg";
    pub(crate) const CHEVRON_LEFT: &str = "icons/chevron-left.svg";
}

/// 打包进二进制的应用资源。
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            icons::TERMINAL => Some(include_bytes!("../../../assets/icons/terminal.svg")),
            icons::FOLDER => Some(include_bytes!("../../../assets/icons/folder.svg")),
            icons::SETTING => Some(include_bytes!("../../../assets/icons/setting.svg")),
            icons::BATCH => Some(include_bytes!("../../../assets/icons/batch.svg")),
            icons::PLUS => Some(include_bytes!("../../../assets/icons/plus.svg")),
            icons::CHEVRON_RIGHT => Some(include_bytes!("../../../assets/icons/chevron-right.svg")),
            icons::CHEVRON_DOWN => Some(include_bytes!("../../../assets/icons/chevron-down.svg")),
            icons::CHEVRON_LEFT => Some(include_bytes!("../../../assets/icons/chevron-left.svg")),
            _ => None,
        };

        match bytes {
            Some(bytes) => Ok(Some(Cow::Borrowed(bytes))),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
