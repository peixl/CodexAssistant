fn main() {
    #[cfg(windows)]
    {
        // Only embed the admin manifest into release artifacts.
        //
        // The packaged launcher must run as administrator (firewall self-heal,
        // hosts edits, loopback bind), so the embedded manifest declares
        // `requireAdministrator`. But during `cargo test` and `cargo run`,
        // Cargo executes the resulting binary directly — and Windows refuses
        // to spawn a UAC-elevated child from a non-elevated shell, surfacing
        // as `os error 740`. Skipping the resource on debug profiles keeps
        // local development friction-free without weakening the shipped exe.
        let profile = std::env::var("PROFILE").unwrap_or_default();
        if profile != "release" {
            return;
        }
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../codex-assistant-manager/src-tauri/icons/icon.ico");
        resource.set_manifest(include_str!(
            "../codex-assistant-manager/src-tauri/windows-app-manifest.xml"
        ));
        let version = env!("CARGO_PKG_VERSION");
        resource.set("ProductName", "CodexAssistant");
        resource.set("ProductVersion", version);
        resource.set("FileVersion", version);
        resource.set("FileDescription", "CodexAssistant Launcher");
        resource.set("InternalName", "codex-assistant");
        resource.set("OriginalFilename", "codex-assistant.exe");
        resource.set("CompanyName", "IFQ.AI");
        resource.set("LegalCopyright", "\u{00A9} IFQ.AI");
        resource.compile().expect("compile launcher resource");
    }
}
