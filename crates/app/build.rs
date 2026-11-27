//! 仅在 Windows 目标把 `assets/branding/app.ico` 嵌入 exe，其他平台不跑资源编译。

fn main() {
    println!("cargo:rerun-if-changed=../../assets/branding/app.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../../assets/branding/app.ico");
    resource.set("ProductName", "Managi");
    resource.set("FileDescription", "Managi GPUI SSH Client");
    resource.set("LegalCopyright", "MIT License");
    if let Err(error) = resource.compile() {
        // 图标只是锦上添花；资源编译失败不能让整个发布流水线失败。
        println!("cargo:warning=嵌入 Windows 图标失败: {error}");
    }
}
