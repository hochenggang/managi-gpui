//! 日志初始化。
//!
//! M0 先输出到 stderr；M1 增加按大小滚动的文件日志（PLAN §4.6）。

pub fn init() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
}
