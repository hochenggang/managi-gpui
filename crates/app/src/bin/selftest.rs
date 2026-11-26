//! 控制台自检入口，供 CI 在三平台运行。

fn main() {
    if let Err(error) = managi_app::selftest::run() {
        eprintln!("selftest failed: {error}");
        std::process::exit(1);
    }
}
