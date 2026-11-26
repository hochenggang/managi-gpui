//! 图形界面入口。Release 构建不分配控制台窗口。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = managi_app::entry(std::env::args().skip(1)) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
