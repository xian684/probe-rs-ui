fn main() {
    // 仅 Windows 目标嵌入图标与版本信息资源，
    // 补充元数据可显著降低安全软件对未签名 Rust 二进制的启发式误报。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set_language(0x0804); // 中文（简体）
        res.compile().expect("failed to compile Windows resource");
    }
}
