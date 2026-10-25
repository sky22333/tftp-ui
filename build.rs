fn main() {
    slint_build::compile("ui/app.slint").expect("Slint 界面编译失败");

    // 把图标写进 exe 资源段：资源管理器/任务栏显示的就是它（窗口图标由 Slint 的 Window.icon 提供）
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=app.rc");
        println!("cargo:rerun-if-changed=assets/icon.ico");
        // embed-resource 3.x 返回 CompilationResult，#[must_use]；
        // 用 manifest_required 让资源嵌入失败时直接中断构建，而不是静默产出无图标的 exe
        embed_resource::compile("app.rc", embed_resource::NONE)
            .manifest_required()
            .expect("图标资源嵌入失败");
    }
}
