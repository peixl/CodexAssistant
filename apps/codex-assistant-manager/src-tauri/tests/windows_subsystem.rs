#[cfg(windows)]
#[test]
fn manager_binary_uses_windows_gui_subsystem_in_debug_and_release() {
    let main_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .expect("read manager main.rs");

    assert!(
        main_rs.contains("#![cfg_attr(windows, windows_subsystem = \"windows\")]"),
        "manager binary should not allocate a console window on Windows"
    );
}

#[test]
fn manager_release_binary_uses_embedded_frontend_assets() {
    let cargo_toml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("read manager Cargo.toml");

    assert!(
        cargo_toml.contains("custom-protocol"),
        "release manager binary should use Tauri custom protocol instead of devUrl localhost"
    );
}

#[test]
fn manager_uses_single_instance_guard_before_starting_tauri() {
    let lib_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read manager lib.rs");

    assert!(lib_rs.contains("acquire_single_instance_guard()"));
    assert!(lib_rs.contains("MANAGER_GUARD_PORT"));
    assert!(lib_rs.contains("manager.already_running"));
}

#[test]
fn launcher_binary_embeds_codex_icon_resource() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let launcher_build = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("codex-assistant-launcher/build.rs");
    let build_rs = std::fs::read_to_string(&launcher_build).expect("read launcher build.rs");

    assert!(build_rs.contains("WindowsResource"));
    assert!(build_rs.contains("icons/icon.ico"));
}

#[test]
fn windows_binaries_request_administrator_for_firewall_self_heal() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manager_build =
        std::fs::read_to_string(manifest_dir.join("build.rs")).expect("read manager build.rs");
    let windows_manifest = std::fs::read_to_string(manifest_dir.join("windows-app-manifest.xml"))
        .expect("read windows app manifest");
    let launcher_build = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("codex-assistant-launcher/build.rs");
    let launcher_build = std::fs::read_to_string(&launcher_build).expect("read launcher build.rs");
    let windows_installer = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("scripts/installer/windows/CodexAssistant.nsi");
    let windows_installer =
        std::fs::read_to_string(&windows_installer).expect("read windows installer");

    assert!(manager_build.contains("windows-app-manifest.xml"));
    assert!(launcher_build.contains("windows-app-manifest.xml"));
    assert!(windows_manifest.contains("Microsoft.Windows.Common-Controls"));
    assert!(
        windows_manifest.contains("level=\"requireAdministrator\""),
        "manifest must request administrator so netsh firewall self-heal succeeds without an extra UAC shell-out per launch"
    );
    assert!(
        !windows_manifest.contains("level=\"asInvoker\""),
        "manifest must not declare asInvoker — Codex.exe firewall rules require admin to add"
    );
    assert!(
        windows_installer.contains("RequestExecutionLevel user"),
        "installer itself stays per-user (writes to %LOCALAPPDATA%) — runtime elevation is the manifest's job"
    );
}

#[test]
fn manager_launch_button_spawns_silent_launcher_binary() {
    let commands_rs =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands.rs"))
            .expect("read manager commands.rs");

    assert!(commands_rs.contains("SILENT_BINARY"));
    assert!(commands_rs.contains("std::process::Command::new"));
    assert!(!commands_rs.contains("launch_and_inject_with_hooks(options"));
}

#[test]
fn macos_packager_hides_silent_launcher_but_not_manager() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packager = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("scripts/installer/macos/package-dmg.sh");
    let script = std::fs::read_to_string(&packager).expect("read macOS packager");

    assert!(script.contains("<key>LSUIElement</key>"));
    assert!(script.contains("ARCH=\"${2:-$(uname -m)}\""));
    assert!(script.contains("BINARY_DIR=\"${BINARY_DIR:-$ROOT/target/release}\""));
    assert!(script.contains("CodexAssistant-${VERSION}-macos-${ARCH}.dmg"));
    assert!(script.contains(
        "create_app \"CodexAssistant\" \"CodexAssistant\" \"$BINARY_DIR/codex-assistant\" \"ai.ifq.codexassistant\" \"true\""
    ));
    assert!(script.contains(
        "create_app \"CodexAssistant 管理工具\" \"CodexAssistantManager\" \"$BINARY_DIR/codex-assistant-manager\" \"ai.ifq.codexassistant.manager\" \"false\""
    ));
}

#[test]
fn github_release_workflow_builds_arm64_dmg_and_windows_installer() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(".github/workflows/release-assets.yml");
    let workflow = std::fs::read_to_string(&workflow).expect("read release assets workflow");

    assert!(workflow.contains("macos-14"));
    assert!(workflow.contains("aarch64-apple-darwin"));
    assert!(workflow.contains("x86_64-pc-windows-msvc"));
    assert!(workflow.contains("package-dmg.sh \"$VERSION\" \"$ARCH\""));
    assert!(workflow.contains("target/aarch64-apple-darwin/release"));
    assert!(
        !workflow.contains("macos-15-intel"),
        "macOS x64 was removed; only arm64 DMG should be built"
    );
    assert!(
        !workflow.contains("x86_64-apple-darwin"),
        "macOS x64 was removed; the workflow must not target Intel anymore"
    );
}

#[test]
fn github_release_workflow_uploads_static_latest_json() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(".github/workflows/release-assets.yml");
    let workflow = std::fs::read_to_string(&workflow).expect("read release assets workflow");

    assert!(workflow.contains("tags: ['v*']"));
    assert!(workflow.contains("TAG=\"${GITHUB_REF_NAME}\""));
    assert!(workflow.contains("ensure-release:"));
    assert!(workflow.contains("GH_REPO: ${{ github.repository }}"));
    assert!(workflow.contains("gh release create \"$TAG\" --verify-tag"));
    assert!(workflow.contains("latest-json:"));
    assert!(workflow.contains("needs.macos.result == 'success'"));
    assert!(workflow.contains("needs.windows.result == 'success'"));
    assert!(workflow.contains("latest.json"));
    assert!(workflow.contains("gh release download \"$TAG\""));
    assert!(workflow.contains(
        "shasum -a 256 \"release-assets/CodexAssistant-${VERSION}-windows-x64-setup.exe\""
    ));
    assert!(
        workflow
            .contains("shasum -a 256 \"release-assets/CodexAssistant-${VERSION}-macos-arm64.dmg\"")
    );
    assert!(workflow.contains("\"sha256\": \"$WIN_SHA\""));
    assert!(workflow.contains("\"sha256\": \"$MAC_SHA\""));
    assert!(workflow.contains("gh release upload \"$TAG\" latest.json --clobber"));
}

#[test]
fn relay_settings_keeps_profile_config_and_auth_files_isolated() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_root = manifest_dir.parent().unwrap().join("src");
    let account_drawer =
        std::fs::read_to_string(src_root.join("drawers/AccountDrawer.tsx")).expect("AccountDrawer");
    let relay_panel = std::fs::read_to_string(src_root.join("panels/RelayAdvancedPanel.tsx"))
        .expect("RelayAdvancedPanel");
    let commands_rs = manifest_dir.join("src/commands.rs");
    let commands_rs = std::fs::read_to_string(&commands_rs).expect("read manager commands.rs");

    // Front-end must route mode switches to the matching Tauri command.
    assert!(account_drawer.contains("apply_pure_api_injection"));
    assert!(account_drawer.contains("apply_relay_injection"));

    // The advanced relay editor must still write config + auth via save_relay_file.
    assert!(relay_panel.contains("configContents"));
    assert!(relay_panel.contains("authContents"));
    assert!(relay_panel.contains("save_relay_file"));

    // Back-end isolation guarantee remains.
    assert!(!commands_rs.contains("缺少独立 auth.json"));
    assert!(commands_rs.contains("apply_relay_files_to_home"));
}
