fn main() {
    slint_build::compile("ui/app.slint").expect("Slint 界面编译失败");

    // 图标写入 exe 资源段（窗口图标由 Slint 的 Window.icon 提供）
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=app.rc");
        println!("cargo:rerun-if-changed=assets/icon.ico");
        // manifest_required 让嵌入失败中断构建，而不是静默产出无图标的 exe
        embed_resource::compile("app.rc", embed_resource::NONE)
            .manifest_required()
            .expect("图标资源嵌入失败");
    }
}
