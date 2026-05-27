use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use codex_assistant_core::app_paths::{
    build_codex_executable, codex_app_version, find_latest_codex_app_dir,
    find_latest_codex_app_dir_from_roots, find_macos_codex_app, normalize_codex_app_path,
    packaged_app_user_model_id, resolve_codex_app_dir_with_saved, user_data_candidates_from,
};
use codex_assistant_core::launcher::{
    CodexLaunch, DefaultLaunchHooks, LaunchHooks, LaunchOptions, MacosCleanupPolicy,
    apply_protocol_proxy_fallback_config_for_launch, build_codex_arguments, build_codex_command,
    build_macos_cleanup_command, build_macos_open_command, build_packaged_activation,
    codex_process_environment_from, launch_and_inject_with_hooks, with_temporary_proxy_environment,
};
#[cfg(windows)]
use codex_assistant_core::launcher::{
    WindowsProcessControlStrategy, windows_process_control_strategy,
};
use codex_assistant_core::ports::select_platform_loopback_port_with;
use codex_assistant_core::proxy::has_proxy_environment;
use codex_assistant_core::settings::{BackendSettings, RelayProfile, RelayProtocol};
use codex_assistant_core::status::StatusStore;

#[test]
fn app_paths_find_latest_windows_package_prefers_highest_version_app_dir() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("OpenAI.Codex_1.2.3.0_x64__abc/app")).unwrap();
    std::fs::create_dir_all(temp.path().join("OpenAI.Codex_26.429.8261.0_x64__abc/app")).unwrap();
    std::fs::create_dir_all(temp.path().join("OpenAI.Codex_not-a-version_x64__abc")).unwrap();

    let latest = find_latest_codex_app_dir(temp.path()).unwrap();

    assert_eq!(
        latest,
        temp.path().join("OpenAI.Codex_26.429.8261.0_x64__abc/app")
    );
}

#[test]
fn app_paths_find_latest_windows_package_returns_package_when_app_dir_missing() {
    let temp = tempfile::tempdir().unwrap();
    let package = temp.path().join("OpenAI.Codex_26.429.8261.0_x64__abc");
    std::fs::create_dir_all(&package).unwrap();

    assert_eq!(find_latest_codex_app_dir(temp.path()).unwrap(), package);
}

#[test]
fn app_paths_find_latest_windows_package_checks_roots_before_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("WindowsApps");
    std::fs::create_dir_all(root.join("OpenAI.Codex_1.0.0.0_x64__abc/app")).unwrap();
    std::fs::create_dir_all(root.join("OpenAI.Codex_26.513.3673.0_x64__abc/app")).unwrap();

    let latest = find_latest_codex_app_dir_from_roots(&[root]).unwrap();

    assert!(latest.ends_with("OpenAI.Codex_26.513.3673.0_x64__abc/app"));
}

#[test]
fn app_paths_extracts_codex_version_from_windows_package_app_dir() {
    let app_dir =
        PathBuf::from(r"C:\Program Files\WindowsApps\OpenAI.Codex_26.513.3673.0_x64__abc\app");

    assert_eq!(
        codex_app_version(&app_dir).as_deref(),
        Some("26.513.3673.0")
    );
}

#[test]
fn app_paths_extracts_codex_version_from_macos_bundle_plist() {
    let temp = tempfile::tempdir().unwrap();
    let app = temp.path().join("OpenAI Codex.app");
    let contents = app.join("Contents");
    std::fs::create_dir_all(&contents).unwrap();
    std::fs::write(
        contents.join("Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>CFBundleVersion</key>
  <string>26.500.0</string>
  <key>CFBundleShortVersionString</key>
  <string>26.513.3673</string>
</dict>
</plist>
"#,
    )
    .unwrap();

    assert_eq!(codex_app_version(&app).as_deref(), Some("26.513.3673"));
}

#[test]
fn app_paths_user_data_candidates_include_local_and_roaming_variants() {
    let local = PathBuf::from(r"C:\Users\me\AppData\Local");
    let roaming = PathBuf::from(r"C:\Users\me\AppData\Roaming");

    let candidates = user_data_candidates_from(Some(&local), Some(&roaming));

    assert_eq!(
        candidates,
        vec![
            local.join("OpenAI").join("Codex"),
            local.join("OpenAI.Codex"),
            local.join("Codex"),
            roaming.join("OpenAI").join("Codex"),
            roaming.join("OpenAI.Codex"),
            roaming.join("Codex"),
        ]
    );
}

#[test]
fn app_paths_find_macos_codex_app_prefers_first_search_root_and_known_names() {
    let temp = tempfile::tempdir().unwrap();
    let system_root = temp.path().join("Applications");
    let user_root = temp.path().join("Users/me/Applications");
    let system_app = system_root.join("OpenAI Codex.app");
    let user_app = user_root.join("Codex.app");
    std::fs::create_dir_all(&system_app).unwrap();
    std::fs::create_dir_all(&user_app).unwrap();

    assert_eq!(
        find_macos_codex_app(&[system_root, user_root]).unwrap(),
        system_app
    );
}

#[test]
fn app_paths_build_macos_bundle_executable() {
    let app = PathBuf::from("/Applications/OpenAI Codex.app");

    assert_eq!(
        build_codex_executable(&app),
        PathBuf::from("/Applications/OpenAI Codex.app/Contents/MacOS/Codex")
    );
}

#[test]
fn app_paths_normalizes_executable_and_package_paths() {
    let temp = tempfile::tempdir().unwrap();
    let portable = temp.path().join("CodexPortable");
    let app = portable.join("app");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::write(app.join("Codex.exe"), "").unwrap();

    assert_eq!(
        normalize_codex_app_path(&app.join("Codex.exe")).as_deref(),
        Some(app.as_path())
    );
    assert_eq!(
        normalize_codex_app_path(&portable).as_deref(),
        Some(app.as_path())
    );
}

#[test]
fn app_paths_saved_path_is_used_when_no_explicit_path_is_provided() {
    let temp = tempfile::tempdir().unwrap();
    let app = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app).unwrap();

    assert_eq!(
        resolve_codex_app_dir_with_saved(None, Some(&app.to_string_lossy())).as_deref(),
        Some(app.as_path())
    );
}

#[test]
fn launcher_builds_debug_arguments_and_commands() {
    let app_dir = PathBuf::from(r"C:\Codex\app");

    assert_eq!(
        build_codex_arguments(9229, &[]),
        vec![
            "--remote-debugging-port=9229".to_string(),
            "--remote-allow-origins=http://127.0.0.1:9229".to_string(),
        ]
    );
    let command = build_codex_command(&app_dir, 9229, &[]);
    assert_eq!(command[1], "--remote-debugging-port=9229");
    assert_eq!(command[2], "--remote-allow-origins=http://127.0.0.1:9229");
}

#[test]
fn launcher_appends_extra_codex_arguments_after_debug_arguments() {
    let app_dir = PathBuf::from(r"C:\Codex\app");
    let extra_args = vec![
        "--force_high_performance_gpu".to_string(),
        "  ".to_string(),
        "--enable-features=UseOzonePlatform".to_string(),
    ];

    assert_eq!(
        build_codex_arguments(9229, &extra_args),
        vec![
            "--remote-debugging-port=9229".to_string(),
            "--remote-allow-origins=http://127.0.0.1:9229".to_string(),
            "--force_high_performance_gpu".to_string(),
            "--enable-features=UseOzonePlatform".to_string(),
        ]
    );
    let command = build_codex_command(&app_dir, 9229, &extra_args);
    assert_eq!(command[1], "--remote-debugging-port=9229");
    assert_eq!(command[2], "--remote-allow-origins=http://127.0.0.1:9229");
    assert_eq!(command[3], "--force_high_performance_gpu");
    assert_eq!(command[4], "--enable-features=UseOzonePlatform");
}

#[test]
fn launcher_constructs_windows_packaged_activation_without_real_app() {
    let app_dir = PathBuf::from(
        r"C:\Program Files\WindowsApps\OpenAI.Codex_26.506.2212.0_x64__2p2nqsd0c76g0\app",
    );

    assert_eq!(
        packaged_app_user_model_id(&app_dir).unwrap(),
        "OpenAI.Codex_2p2nqsd0c76g0!App"
    );
    assert_eq!(
        build_packaged_activation(&app_dir, 9229, &[]).unwrap(),
        CodexLaunch::PackagedActivation {
            app_user_model_id: "OpenAI.Codex_2p2nqsd0c76g0!App".to_string(),
            arguments: "--remote-debugging-port=9229 --remote-allow-origins=http://127.0.0.1:9229"
                .to_string(),
            process_id: None,
        }
    );
}

#[test]
fn launcher_packaged_activation_appends_extra_codex_arguments() {
    let app_dir = PathBuf::from(
        r"C:\Program Files\WindowsApps\OpenAI.Codex_26.506.2212.0_x64__2p2nqsd0c76g0\app",
    );
    let extra_args = vec!["--force_high_performance_gpu".to_string()];

    assert_eq!(
        build_packaged_activation(&app_dir, 9229, &extra_args).unwrap(),
        CodexLaunch::PackagedActivation {
            app_user_model_id: "OpenAI.Codex_2p2nqsd0c76g0!App".to_string(),
            arguments:
                "--remote-debugging-port=9229 --remote-allow-origins=http://127.0.0.1:9229 --force_high_performance_gpu"
                    .to_string(),
            process_id: None,
        }
    );
}

#[test]
fn launcher_packaged_activation_can_preserve_process_id() {
    let launch = CodexLaunch::PackagedActivation {
        app_user_model_id: "OpenAI.Codex_2p2nqsd0c76g0!App".to_string(),
        arguments: "--remote-debugging-port=9229".to_string(),
        process_id: Some(4242),
    };

    assert_eq!(launch.process_id(), Some(4242));
}

#[cfg(windows)]
#[test]
fn launcher_windows_packaged_process_management_uses_native_api() {
    assert_eq!(
        windows_process_control_strategy(),
        WindowsProcessControlStrategy::NativeWindowsApi
    );
}

#[test]
fn launcher_macos_open_command_waits_for_app_exit() {
    let command = build_macos_open_command(Path::new("/Applications/Codex.app"), 9229, &[]);

    assert_eq!(command[0], "open");
    assert!(command.contains(&"-W".to_string()));
    assert!(command.contains(&"-a".to_string()));
    assert!(command.contains(&"--args".to_string()));
    assert!(command.contains(&"--remote-debugging-port=9229".to_string()));
}

#[test]
fn launcher_macos_open_command_appends_extra_codex_arguments_after_args() {
    let extra_args = vec!["--force_high_performance_gpu".to_string()];
    let command = build_macos_open_command(Path::new("/Applications/Codex.app"), 9229, &extra_args);
    let args_index = command
        .iter()
        .position(|part| part == "--args")
        .expect("macOS command should contain --args");

    assert_eq!(
        &command[args_index + 1..],
        &[
            "--remote-debugging-port=9229".to_string(),
            "--remote-allow-origins=http://127.0.0.1:9229".to_string(),
            "--force_high_performance_gpu".to_string(),
        ]
    );
}

#[test]
fn launcher_packaged_activation_temporarily_applies_proxy_environment() {
    temp_env_remove("HTTP_PROXY");
    temp_env_remove("HTTPS_PROXY");
    temp_env_remove("ALL_PROXY");
    temp_env_remove("http_proxy");
    temp_env_remove("https_proxy");
    temp_env_remove("all_proxy");
    temp_env_remove("NO_PROXY");
    temp_env_remove("no_proxy");
    temp_env_set("UNRELATED_PROXY_TEST", "keep");
    let mut env = HashMap::new();
    env.insert(
        "HTTP_PROXY".to_string(),
        "http://proxy.example.test:8080".to_string(),
    );
    env.insert(
        "HTTPS_PROXY".to_string(),
        "http://proxy.example.test:8080".to_string(),
    );
    env.insert(
        "ALL_PROXY".to_string(),
        "http://proxy.example.test:8080".to_string(),
    );
    let env = codex_process_environment_from(&env, || None);

    let seen = with_temporary_proxy_environment(&env, || {
        (
            std::env::var("HTTP_PROXY").ok(),
            std::env::var("HTTPS_PROXY").ok(),
            std::env::var("ALL_PROXY").ok(),
            std::env::var("http_proxy").ok(),
            std::env::var("https_proxy").ok(),
            std::env::var("all_proxy").ok(),
            std::env::var("NO_PROXY").ok(),
            std::env::var("no_proxy").ok(),
        )
    });

    assert_eq!(seen.0.as_deref(), Some("http://proxy.example.test:8080"));
    assert_eq!(seen.1.as_deref(), Some("http://proxy.example.test:8080"));
    assert_eq!(seen.2.as_deref(), Some("http://proxy.example.test:8080"));
    assert_eq!(seen.3.as_deref(), Some("http://proxy.example.test:8080"));
    assert_eq!(seen.4.as_deref(), Some("http://proxy.example.test:8080"));
    assert_eq!(seen.5.as_deref(), Some("http://proxy.example.test:8080"));
    assert!(seen.6.as_deref().unwrap_or_default().contains("127.0.0.1"));
    assert_eq!(seen.6, seen.7);
    assert!(std::env::var("HTTP_PROXY").is_err());
    assert!(std::env::var("HTTPS_PROXY").is_err());
    assert!(std::env::var("ALL_PROXY").is_err());
    assert!(std::env::var("http_proxy").is_err());
    assert!(std::env::var("https_proxy").is_err());
    assert!(std::env::var("all_proxy").is_err());
    assert!(std::env::var("NO_PROXY").is_err());
    assert!(std::env::var("no_proxy").is_err());
    assert_eq!(
        std::env::var("UNRELATED_PROXY_TEST").ok().as_deref(),
        Some("keep")
    );
    temp_env_remove("UNRELATED_PROXY_TEST");
}

#[test]
fn proxy_mirrors_lowercase_environment_and_preserves_loopback_no_proxy() {
    let env = HashMap::from([
        (
            "https_proxy".to_string(),
            "http://lowercase-proxy.example.test:8080".to_string(),
        ),
        ("NO_PROXY".to_string(), "example.test".to_string()),
    ]);

    let process_env = codex_process_environment_from(&env, || {
        panic!("system proxy detection should not run when lowercase env already has proxy")
    });

    assert_eq!(
        process_env.get("HTTPS_PROXY").map(String::as_str),
        Some("http://lowercase-proxy.example.test:8080")
    );
    assert_eq!(
        process_env.get("https_proxy").map(String::as_str),
        Some("http://lowercase-proxy.example.test:8080")
    );
    let no_proxy = process_env.get("NO_PROXY").cloned().unwrap_or_default();
    assert!(no_proxy.contains("example.test"));
    assert!(no_proxy.contains("127.0.0.1"));
    assert!(no_proxy.contains("localhost"));
    assert_eq!(process_env.get("NO_PROXY"), process_env.get("no_proxy"));
}

#[test]
fn ports_falls_back_to_ephemeral_when_requested_is_busy() {
    let selected = select_platform_loopback_port_with(9229, |_| false, || 43001);

    assert_eq!(selected, 43001);
}

#[test]
fn ports_keeps_requested_when_bind_succeeds() {
    let selected = select_platform_loopback_port_with(9229, |_| true, || 43001);

    assert_eq!(selected, 9229);
}

#[test]
fn proxy_uses_existing_environment_before_system_proxy() {
    let env = HashMap::from([(
        "HTTPS_PROXY".to_string(),
        "http://env-proxy.example.test:8080".to_string(),
    )]);
    assert!(has_proxy_environment(&env));
    let process_env = codex_process_environment_from(&env, || {
        panic!("system proxy detection should not run when env already has proxy")
    });
    assert_eq!(
        process_env.get("HTTPS_PROXY").map(String::as_str),
        Some("http://env-proxy.example.test:8080")
    );
}

#[test]
fn proxy_injects_system_proxy_when_environment_is_empty() {
    let env = HashMap::new();
    let process_env = codex_process_environment_from(&env, || {
        Some("http://system-proxy.example.test:8080".to_string())
    });

    assert_eq!(
        process_env.get("HTTP_PROXY").map(String::as_str),
        Some("http://system-proxy.example.test:8080")
    );
    assert_eq!(
        process_env.get("HTTPS_PROXY").map(String::as_str),
        Some("http://system-proxy.example.test:8080")
    );
    assert_eq!(
        process_env.get("ALL_PROXY").map(String::as_str),
        Some("http://system-proxy.example.test:8080")
    );
}

#[tokio::test]
async fn default_helper_serves_backend_status_over_http() {
    if !loopback_available_for_test().await {
        return;
    }

    let hooks = DefaultLaunchHooks::default();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    hooks.start_helper(port).await.unwrap();
    let token = codex_assistant_core::helper_auth::ensure_helper_token();
    let client = loopback_reqwest_client();
    let response = client
        .post(format!("http://127.0.0.1:{port}/backend/status"))
        .header("X-Codex-Helper-Token", token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let payload: serde_json::Value = response.json().await.unwrap();
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["transport"], "http-helper");

    let repair_response = client
        .post(format!("http://127.0.0.1:{port}/backend/repair"))
        .header("X-Codex-Helper-Token", token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert!(repair_response.status().is_success());
    let repair_payload: serde_json::Value = repair_response.json().await.unwrap();
    assert_eq!(repair_payload["status"], "ok");
    assert_eq!(repair_payload["transport"], "http-helper");

    hooks.shutdown_helper(port).await;
}

#[tokio::test]
async fn default_helper_accepts_diagnostic_log_events_over_http() {
    if !loopback_available_for_test().await {
        return;
    }

    let temp = tempfile::tempdir().unwrap();
    let log_path = temp.path().join("codex-assistant.log");
    codex_assistant_core::diagnostic_log::set_diagnostic_log_path_for_tests(Some(log_path.clone()));
    let hooks = DefaultLaunchHooks::default();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    hooks.start_helper(port).await.unwrap();
    let token = codex_assistant_core::helper_auth::ensure_helper_token();
    let response = loopback_reqwest_client()
        .post(format!("http://127.0.0.1:{port}/diagnostics/log"))
        .header("X-Codex-Helper-Token", token)
        .json(&serde_json::json!({
            "event": "backend_check_failed",
            "message": "fetch failed",
            "helperBase": format!("http://127.0.0.1:{port}")
        }))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    let payload: serde_json::Value = response.json().await.unwrap();
    assert_eq!(payload["status"], "ok");
    hooks.shutdown_helper(port).await;

    let contents = std::fs::read_to_string(&log_path).unwrap();
    assert!(contents.contains("renderer.backend_check_failed"));
    assert!(contents.contains("fetch failed"));
    codex_assistant_core::diagnostic_log::set_diagnostic_log_path_for_tests(None);
}

async fn loopback_available_for_test() -> bool {
    if loopback_tcp_available() {
        return true;
    }
    eprintln!("skipping loopback-dependent helper test because 127.0.0.1 TCP is unavailable");
    false
}

fn loopback_reqwest_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}

fn loopback_tcp_available() -> bool {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        return false;
    };
    if listener.set_nonblocking(true).is_err() {
        return false;
    }
    let Ok(address) = listener.local_addr() else {
        return false;
    };
    let thread = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        while started.elapsed() < Duration::from_millis(500) {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(b"ok");
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let available = can_read_loopback_probe(address);
    let _ = thread.join();
    available
}

fn can_read_loopback_probe(address: SocketAddr) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(250)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let mut buffer = [0u8; 2];
    stream.read_exact(&mut buffer).is_ok() && buffer == *b"ok"
}

#[tokio::test]
async fn launch_lifecycle_runs_sync_before_launch_writes_success_and_shutdowns_on_exit() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks = FakeHooks::new(events.clone())
        .with_settings(BackendSettings {
            provider_sync_enabled: true,
            ..BackendSettings::default()
        })
        .with_launch_result(CodexLaunch::Process {
            command: vec!["codex".to_string()],
            wait_strategy: codex_assistant_core::launcher::ProcessWaitStrategy::TrackedChild,
            macos_cleanup_policy: None,
        });

    let handle = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir.clone()),
            debug_port: 9229,
            helper_port: 57321,
            status_store,
        },
        &hooks,
    )
    .await
    .unwrap();
    handle.wait_for_codex_exit().await.unwrap();

    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "select-debug:9229",
            "select-helper:57321",
            "load-settings",
            "provider-sync",
            "start-helper:57321",
            "launch:9229",
            "inject:9229:57321",
            "status:running",
            "wait-codex",
            "shutdown-helper:57321",
        ]
    );
    assert_eq!(
        handle
            .status_store
            .load_latest()
            .unwrap()
            .unwrap()
            .codex_app
            .as_deref(),
        Some(app_dir.to_string_lossy().as_ref())
    );
}

#[tokio::test]
async fn launch_lifecycle_passes_configured_extra_args_to_codex_launch() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks = FakeHooks::new(events.clone()).with_settings(BackendSettings {
        codex_extra_args: vec!["--force_high_performance_gpu".to_string()],
        ..BackendSettings::default()
    });

    let handle = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 57321,
            status_store,
        },
        &hooks,
    )
    .await
    .unwrap();
    handle.wait_for_codex_exit().await.unwrap();

    assert!(
        events
            .lock()
            .unwrap()
            .contains(&"launch:9229:--force_high_performance_gpu".to_string())
    );
}

#[tokio::test]
async fn launch_lifecycle_keeps_js_injection_in_relay_mode() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks = FakeHooks::new(events.clone()).with_settings(BackendSettings {
        launch_mode: codex_assistant_core::settings::LaunchMode::Relay,
        ..BackendSettings::default()
    });

    let handle = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 57321,
            status_store,
        },
        &hooks,
    )
    .await
    .unwrap();
    handle.wait_for_codex_exit().await.unwrap();

    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "select-debug:9229",
            "select-helper:57321",
            "load-settings",
            "start-helper:57321",
            "launch:9229",
            "inject:9229:57321",
            "status:running",
            "wait-codex",
            "shutdown-helper:57321",
        ]
    );
}

#[tokio::test]
async fn launch_lifecycle_skips_helper_and_injection_when_enhancements_disabled() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks = FakeHooks::new(events.clone()).with_settings(BackendSettings {
        enhancements_enabled: false,
        ..BackendSettings::default()
    });

    let handle = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 57321,
            status_store,
        },
        &hooks,
    )
    .await
    .unwrap();
    handle.wait_for_codex_exit().await.unwrap();

    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "select-debug:9229",
            "select-helper:57321",
            "load-settings",
            "launch:9229",
            "status:running",
            "wait-codex",
        ]
    );
}

#[tokio::test]
async fn launch_lifecycle_degrades_instead_of_blocking_when_loopback_preflight_fails() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks = FakeHooks::new(events.clone()).with_loopback_error("loopback blocked");

    let handle = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 57321,
            status_store: status_store.clone(),
        },
        &hooks,
    )
    .await
    .expect("loopback failure must not prevent opening Codex");
    handle.wait_for_codex_exit().await.unwrap();

    let observed = events.lock().unwrap().clone();
    assert!(
        observed.contains(&"launch:9229".to_string()),
        "Codex launch must still be attempted: {observed:?}"
    );
    assert!(
        !observed.contains(&"start-helper:57321".to_string()),
        "helper cannot be started when loopback is known bad: {observed:?}"
    );
    assert!(
        !observed.contains(&"inject:9229:57321".to_string()),
        "CDP injection should be skipped when loopback is known bad: {observed:?}"
    );
    assert!(
        observed.contains(&"status:running_degraded".to_string()),
        "expected degraded status after loopback failure: {observed:?}"
    );
    let status = status_store.load_latest().unwrap().unwrap();
    assert_eq!(status.status, "running_degraded");
    assert!(
        status.message.contains("loopback blocked"),
        "degraded status should include loopback diagnostic: {}",
        status.message
    );
}

#[tokio::test]
async fn launch_lifecycle_applies_direct_chat_fallback_when_proxy_loopback_fails() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks = FakeHooks::new(events.clone())
        .with_loopback_error("loopback blocked")
        .with_settings(BackendSettings {
            enhancements_enabled: false,
            relay_profiles: vec![RelayProfile {
                id: "relay-chat".to_string(),
                name: "Chat".to_string(),
                base_url: "https://chat-only.example.test/v1".to_string(),
                api_key: "sk-test".to_string(),
                protocol: RelayProtocol::ChatCompletions,
                relay_mode: codex_assistant_core::settings::RelayMode::MixedApi,
                official_mix_api_key: false,
                test_model: String::new(),
                config_contents: String::new(),
                auth_contents: String::new(),
            }],
            active_relay_id: "relay-chat".to_string(),
            ..BackendSettings::default()
        });

    let handle = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 58000,
            status_store: status_store.clone(),
        },
        &hooks,
    )
    .await
    .expect("loopback failure must not prevent opening Codex");
    handle.wait_for_codex_exit().await.unwrap();

    let observed = events.lock().unwrap().clone();
    assert!(
        observed.contains(&"protocol-fallback:57321".to_string()),
        "chat relay should switch away from the local proxy when loopback is blocked: {observed:?}"
    );
    assert!(
        !observed.contains(&"start-helper:57321".to_string()),
        "helper cannot be started when loopback is known bad: {observed:?}"
    );
    assert!(
        observed.contains(&"launch:9229".to_string()),
        "Codex launch must still be attempted: {observed:?}"
    );
    let status = status_store.load_latest().unwrap().unwrap();
    assert_eq!(status.status, "running_degraded");
    assert!(status.message.contains("loopback blocked"));
    assert!(
        status.message.contains("direct Chat Completions wire API"),
        "degraded status should explain the relay fallback: {}",
        status.message
    );
}

#[test]
fn launch_fallback_rewrites_chat_protocol_config_to_direct_when_loopback_unavailable() {
    let temp = tempfile::tempdir().unwrap();
    let settings = BackendSettings {
        relay_profiles: vec![RelayProfile {
            id: "relay-chat".to_string(),
            name: "Chat".to_string(),
            base_url: "https://chat-only.example.test/v1".to_string(),
            api_key: "sk-test".to_string(),
            protocol: RelayProtocol::ChatCompletions,
            relay_mode: codex_assistant_core::settings::RelayMode::MixedApi,
            official_mix_api_key: false,
            test_model: String::new(),
            config_contents: String::new(),
            auth_contents: String::new(),
        }],
        active_relay_id: "relay-chat".to_string(),
        ..BackendSettings::default()
    };

    let result = apply_protocol_proxy_fallback_config_for_launch(temp.path(), &settings, 57321)
        .unwrap()
        .expect("chat relay with credentials should produce a direct fallback config");
    let updated = std::fs::read_to_string(temp.path().join("config.toml")).unwrap();

    assert!(result.configured);
    assert!(updated.contains(r#"wire_api = "chat""#));
    assert!(updated.contains(r#"base_url = "https://chat-only.example.test/v1""#));
    assert!(!updated.contains("127.0.0.1:57321"));
}

#[test]
fn launch_fallback_preserves_custom_relay_file_profiles() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("config.toml"), "model = \"old\"\n").unwrap();
    let settings = BackendSettings {
        relay_profiles: vec![RelayProfile {
            id: "relay-chat".to_string(),
            name: "Chat".to_string(),
            base_url: "https://chat-only.example.test/v1".to_string(),
            api_key: "sk-test".to_string(),
            protocol: RelayProtocol::ChatCompletions,
            relay_mode: codex_assistant_core::settings::RelayMode::MixedApi,
            official_mix_api_key: false,
            test_model: String::new(),
            config_contents: "model_provider = \"custom\"".to_string(),
            auth_contents: String::new(),
        }],
        active_relay_id: "relay-chat".to_string(),
        ..BackendSettings::default()
    };

    let result = apply_protocol_proxy_fallback_config_for_launch(temp.path(), &settings, 57321)
        .expect("custom file profiles should skip fallback without error");

    assert!(result.is_none());
    assert_eq!(
        std::fs::read_to_string(temp.path().join("config.toml")).unwrap(),
        "model = \"old\"\n"
    );
}

#[tokio::test]
async fn launch_lifecycle_keeps_codex_running_and_marks_degraded_when_injection_fails() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks = FakeHooks::new(events.clone()).with_inject_error("inject failed");

    let handle = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 57321,
            status_store: status_store.clone(),
        },
        &hooks,
    )
    .await
    .expect("injection failure should not abort launch — Codex must keep running");

    drop(handle);
    let observed = events.lock().unwrap().clone();
    assert!(
        !observed.contains(&"terminate-codex".to_string()),
        "Codex must not be terminated when only CDP injection fails: {observed:?}"
    );
    assert!(
        !observed.contains(&"shutdown-helper:57321".to_string()),
        "helper must remain available when only CDP injection fails: {observed:?}"
    );
    assert!(
        observed.contains(&"status:running_degraded".to_string()),
        "expected running_degraded status event: {observed:?}"
    );
    let status = status_store.load_latest().unwrap().unwrap();
    assert_eq!(status.status, "running_degraded");
    assert!(
        status.message.contains("inject failed"),
        "degraded message should include the underlying error: {}",
        status.message
    );
}

#[tokio::test]
async fn launch_lifecycle_cleans_helper_when_launch_fails_after_helper_started() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks = FakeHooks::new(events.clone()).with_launch_error("launch failed");

    let error = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 57321,
            status_store: status_store.clone(),
        },
        &hooks,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("launch failed"));
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "select-debug:9229",
            "select-helper:57321",
            "load-settings",
            "start-helper:57321",
            "launch:9229",
            "shutdown-helper:57321",
            "status:failed",
        ]
    );
}

#[tokio::test]
async fn launch_starts_helper_when_chat_protocol_proxy_is_enabled() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let settings = BackendSettings {
        enhancements_enabled: false,
        relay_profiles: vec![RelayProfile {
            id: "relay-chat".to_string(),
            name: "Chat".to_string(),
            base_url: "https://chat-only.example.test/v1".to_string(),
            api_key: "sk-test".to_string(),
            protocol: RelayProtocol::ChatCompletions,
            relay_mode: codex_assistant_core::settings::RelayMode::MixedApi,
            official_mix_api_key: false,
            test_model: String::new(),
            config_contents: String::new(),
            auth_contents: String::new(),
        }],
        active_relay_id: "relay-chat".to_string(),
        ..BackendSettings::default()
    };
    let hooks = FakeHooks::new(events.clone()).with_settings(settings);

    let handle = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 58000,
            status_store,
        },
        &hooks,
    )
    .await
    .unwrap();

    let before_stop = events.lock().unwrap().clone();
    assert!(before_stop.contains(&"select-helper:58000".to_string()));
    assert!(before_stop.contains(&"start-helper:57321".to_string()));
    assert!(!before_stop.contains(&"inject:9229:57321".to_string()));

    handle.wait_for_codex_exit().await.unwrap();

    let after_stop = events.lock().unwrap().clone();
    assert!(after_stop.contains(&"wait-codex".to_string()));
    assert!(after_stop.contains(&"shutdown-helper:57321".to_string()));
}

#[tokio::test]
async fn launch_lifecycle_cleans_helper_and_codex_when_status_save_fails() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(temp.path().join("status-parent-file"), "not a directory").unwrap();
    let status_store = StatusStore::new(
        temp.path()
            .join("status-parent-file")
            .join("latest-status.json"),
    );
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks =
        FakeHooks::new(events.clone()).with_launch_result(CodexLaunch::PackagedActivation {
            app_user_model_id: "OpenAI.Codex_2p2nqsd0c76g0!App".to_string(),
            arguments: "--remote-debugging-port=9229".to_string(),
            process_id: Some(4242),
        });

    let error = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 57321,
            status_store,
        },
        &hooks,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("failed to create directory"));
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "select-debug:9229",
            "select-helper:57321",
            "load-settings",
            "start-helper:57321",
            "launch:9229",
            "inject:9229:57321",
            "shutdown-helper:57321",
            "terminate-packaged:4242",
            "status:failed",
        ]
    );
}

#[tokio::test]
async fn launch_lifecycle_keeps_packaged_codex_running_when_injection_fails() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join("Codex.app");
    std::fs::create_dir_all(&app_dir).unwrap();
    let status_store = StatusStore::new(temp.path().join("latest-status.json"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let hooks = FakeHooks::new(events.clone())
        .with_launch_result(CodexLaunch::PackagedActivation {
            app_user_model_id: "OpenAI.Codex_2p2nqsd0c76g0!App".to_string(),
            arguments: "--remote-debugging-port=9229".to_string(),
            process_id: Some(4242),
        })
        .with_inject_error("inject failed");

    let handle = launch_and_inject_with_hooks(
        LaunchOptions {
            app_dir: Some(app_dir),
            debug_port: 9229,
            helper_port: 57321,
            status_store: status_store.clone(),
        },
        &hooks,
    )
    .await
    .expect("packaged Codex must stay running even if CDP injection fails");

    drop(handle);
    let observed = events.lock().unwrap().clone();
    assert!(
        !observed.contains(&"terminate-packaged:4242".to_string()),
        "packaged Codex process must not be killed when only CDP injection fails: {observed:?}"
    );
    let status = status_store.load_latest().unwrap().unwrap();
    assert_eq!(status.status, "running_degraded");
}

#[tokio::test]
async fn default_provider_sync_enabled_fails_instead_of_silently_skipping() {
    let hooks = FakeHooks::new(Arc::new(Mutex::new(Vec::new()))).with_provider_sync_unsupported();

    let error = hooks
        .run_provider_sync()
        .await
        .expect_err("default-style provider sync should be explicit");

    assert!(
        error
            .to_string()
            .contains("provider sync requires launcher hooks")
    );
}

#[test]
fn launcher_macos_cleanup_command_targets_specific_app_bundle() {
    let command = build_macos_cleanup_command(
        Path::new("/Applications/OpenAI Codex.app"),
        MacosCleanupPolicy::QuitIfNotPreviouslyRunning,
    )
    .expect("cleanup command should be allowed");

    assert_eq!(command[0], "osascript");
    assert!(command.iter().any(|part| part.contains("OpenAI Codex")));
    assert!(!command.iter().any(|part| part == "Codex"));
}

#[test]
fn launcher_macos_cleanup_is_skipped_when_app_was_already_running() {
    let command = build_macos_cleanup_command(
        Path::new("/Applications/OpenAI Codex.app"),
        MacosCleanupPolicy::SkipQuitBecauseAlreadyRunning,
    );

    assert_eq!(command, None);
}

#[tokio::test]
async fn default_launch_hooks_provider_sync_enabled_returns_explicit_error() {
    let error = DefaultLaunchHooks::default()
        .run_provider_sync()
        .await
        .expect_err("default provider sync should not silently skip");

    assert!(
        error
            .to_string()
            .contains("provider sync requires launcher hooks")
    );
}

#[derive(Clone)]
struct FakeHooks {
    events: Arc<Mutex<Vec<String>>>,
    settings: BackendSettings,
    launch_result: CodexLaunch,
    launch_error: Option<String>,
    inject_error: Option<String>,
    loopback_error: Option<String>,
    provider_sync_unsupported: bool,
}

impl FakeHooks {
    fn new(events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            events,
            settings: BackendSettings::default(),
            launch_result: CodexLaunch::Process {
                command: vec!["codex".to_string()],
                wait_strategy: codex_assistant_core::launcher::ProcessWaitStrategy::TrackedChild,
                macos_cleanup_policy: None,
            },
            launch_error: None,
            inject_error: None,
            loopback_error: None,
            provider_sync_unsupported: false,
        }
    }

    fn with_settings(mut self, settings: BackendSettings) -> Self {
        self.settings = settings;
        self
    }

    fn with_launch_result(mut self, launch_result: CodexLaunch) -> Self {
        self.launch_result = launch_result;
        self
    }

    fn with_inject_error(mut self, message: &str) -> Self {
        self.inject_error = Some(message.to_string());
        self
    }

    fn with_launch_error(mut self, message: &str) -> Self {
        self.launch_error = Some(message.to_string());
        self
    }

    fn with_loopback_error(mut self, message: &str) -> Self {
        self.loopback_error = Some(message.to_string());
        self
    }

    fn with_provider_sync_unsupported(mut self) -> Self {
        self.provider_sync_unsupported = true;
        self
    }

    fn event(&self, event: impl Into<String>) {
        self.events.lock().unwrap().push(event.into());
    }
}

fn temp_env_set(key: &str, value: &str) {
    unsafe {
        std::env::set_var(key, value);
    }
}

fn temp_env_remove(key: &str) {
    unsafe {
        std::env::remove_var(key);
    }
}

#[async_trait::async_trait(?Send)]
impl LaunchHooks for FakeHooks {
    fn resolve_app_dir(
        &self,
        app_dir: Option<&Path>,
        _settings: &BackendSettings,
    ) -> anyhow::Result<PathBuf> {
        app_dir
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow::anyhow!("missing app dir"))
    }

    fn select_debug_port(&self, requested: u16) -> u16 {
        self.event(format!("select-debug:{requested}"));
        requested
    }

    fn select_helper_port(&self, requested: u16) -> u16 {
        self.event(format!("select-helper:{requested}"));
        requested
    }

    async fn load_settings(&self) -> anyhow::Result<BackendSettings> {
        self.event("load-settings");
        Ok(self.settings.clone())
    }

    async fn run_provider_sync(&self) -> anyhow::Result<()> {
        self.event("provider-sync");
        if self.provider_sync_unsupported {
            anyhow::bail!("provider sync requires launcher hooks");
        }
        Ok(())
    }

    async fn verify_loopback_reachable(&self) -> anyhow::Result<()> {
        if let Some(message) = &self.loopback_error {
            anyhow::bail!(message.clone());
        }
        Ok(())
    }

    async fn apply_protocol_proxy_fallback(
        &self,
        settings: &BackendSettings,
        helper_port: u16,
    ) -> anyhow::Result<Option<codex_assistant_core::relay_config::RelayApplyResult>> {
        let relay = settings.active_relay_profile();
        if relay.protocol == RelayProtocol::ChatCompletions
            && !relay.base_url.trim().is_empty()
            && !relay.api_key.trim().is_empty()
            && relay.config_contents.trim().is_empty()
            && relay.auth_contents.trim().is_empty()
        {
            self.event(format!("protocol-fallback:{helper_port}"));
            return Ok(Some(codex_assistant_core::relay_config::RelayApplyResult {
                config_path: "fake/config.toml".to_string(),
                backup_path: None,
                configured: true,
            }));
        }
        Ok(None)
    }

    async fn start_helper(&self, helper_port: u16) -> anyhow::Result<()> {
        self.event(format!("start-helper:{helper_port}"));
        Ok(())
    }

    async fn launch_codex(
        &self,
        app_dir: &Path,
        debug_port: u16,
        extra_args: &[String],
    ) -> anyhow::Result<CodexLaunch> {
        assert!(app_dir.ends_with("Codex.app"));
        if extra_args.is_empty() {
            self.event(format!("launch:{debug_port}"));
        } else {
            self.event(format!("launch:{debug_port}:{}", extra_args.join(",")));
        }
        if let Some(message) = &self.launch_error {
            anyhow::bail!(message.clone());
        }
        Ok(self.launch_result.clone())
    }

    async fn inject(&self, debug_port: u16, helper_port: u16) -> anyhow::Result<()> {
        self.event(format!("inject:{debug_port}:{helper_port}"));
        if let Some(message) = &self.inject_error {
            anyhow::bail!(message.clone());
        }
        Ok(())
    }

    async fn write_status(&self, status: &str) {
        self.event(format!("status:{status}"));
    }

    async fn wait_for_codex_exit(&self, _launch: &CodexLaunch) -> anyhow::Result<()> {
        self.event("wait-codex");
        Ok(())
    }

    async fn shutdown_helper(&self, helper_port: u16) {
        self.event(format!("shutdown-helper:{helper_port}"));
    }

    async fn terminate_codex(&self, launch: &CodexLaunch) {
        if let Some(process_id) = launch.process_id() {
            self.event(format!("terminate-packaged:{process_id}"));
        } else {
            self.event("terminate-codex");
        }
    }
}
