fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../codex-plus-manager/src-tauri/icons/icon.ico");
        resource.set_manifest(include_str!(
            "../codex-plus-manager/src-tauri/windows-app-manifest.xml"
        ));
        let version = env!("CARGO_PKG_VERSION");
        resource.set("ProductName", "CodexAssistant");
        resource.set("ProductVersion", version);
        resource.set("FileVersion", version);
        resource.set("FileDescription", "CodexAssistant Launcher");
        resource.set("InternalName", "codex-plus-plus");
        resource.set("OriginalFilename", "codex-plus-plus.exe");
        resource.set("CompanyName", "IFQ.AI");
        resource.set("LegalCopyright", "\u{00A9} IFQ.AI");
        resource.compile().expect("compile launcher resource");
    }
}
