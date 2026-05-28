fn main() {
    let mut windows = tauri_build::WindowsAttributes::new();
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile == "release" {
        // The packaged manager runs as administrator so it can drive netsh /
        // hosts edits without re-prompting on every launch. During `cargo test`
        // Cargo invokes the test harness directly, and Windows refuses to spawn
        // a UAC-elevated child from a non-elevated shell — leaving developers
        // staring at `os error 740`. Embedding the admin manifest only for
        // release builds keeps shipped behaviour intact while letting
        // unprivileged shells run the full test suite.
        windows = windows.app_manifest(include_str!("windows-app-manifest.xml"));
    }
    let attrs = tauri_build::Attributes::new().windows_attributes(windows);
    tauri_build::try_build(attrs).expect("failed to run Tauri build script");
}
