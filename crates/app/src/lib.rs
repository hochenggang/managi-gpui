//! 应用装配：命令行入口、窗口创建、无窗口自检。
//!
//! UI 与网络/终端通过独立 crate 解耦；这里只负责把它们接起来。

use std::error::Error;

use gpui_kit::*;
use managi_ui::AppView;

pub mod logging;
pub mod selftest;

/// 解析参数并分派。任何失败都以非零退出码结束。
pub fn entry<I>(args: I) -> Result<(), Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let first = args.next();

    match first.as_deref() {
        None => run_gui(),
        Some("--selftest") => selftest::run(),
        Some("--version" | "-V") => {
            println!("managi-gpui {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some(other) => Err(Box::new(std::io::Error::other(format!(
            "未知参数：{other}（可用 --selftest / --version / --help）"
        )))),
    }
}

fn run_gui() -> Result<(), Box<dyn Error>> {
    logging::init();

    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(window_options(), cx, |_window, cx| {
            cx.new(|_| AppView::new(env!("CARGO_PKG_VERSION")))
        })
        .expect("打开主窗口失败");
    });

    Ok(())
}

/// 窗口尺寸与标题；DESIGN.md §4 规定默认 1280×800。
fn window_options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(100.0), px(80.0)),
            size(px(1280.0), px(800.0)),
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("managi-gpui".into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn print_help() {
    println!("managi-gpui {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("用法:");
    println!("  managi-gpui            打开图形界面");
    println!("  managi-gpui --selftest 无窗口自检（配置/终端/参数）");
    println!("  managi-gpui --version  打印版本");
}
