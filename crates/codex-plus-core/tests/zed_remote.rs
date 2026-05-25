use codex_plus_core::zed_remote::{self, SshTarget, ZedRemoteError};
use serde_json::json;

#[test]
fn build_zed_remote_url_with_user_host_port_and_encoded_path() {
    let url = zed_remote::build_zed_remote_url(
        &SshTarget {
            user: "alice".to_string(),
            host: "example.com".to_string(),
            port: Some(2222),
        },
        "/home/alice/My Project/你好.py",
    )
    .unwrap();

    assert_eq!(
        url,
        "ssh://alice@example.com:2222/home/alice/My%20Project/%E4%BD%A0%E5%A5%BD.py"
    );
}

#[test]
fn build_zed_remote_url_allows_host_without_user() {
    let url = zed_remote::build_zed_remote_url(
        &SshTarget {
            user: String::new(),
            host: "box.internal".to_string(),
            port: None,
        },
        "/srv/app/main.py",
    )
    .unwrap();

    assert_eq!(url, "ssh://box.internal/srv/app/main.py");
}

#[test]
fn build_zed_remote_url_rejects_invalid_inputs() {
    let error = zed_remote::build_zed_remote_url(
        &SshTarget {
            user: "alice".to_string(),
            host: "bad host".to_string(),
            port: None,
        },
        "/a.py",
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ZedRemoteError::Validation("Invalid SSH host")
    ));
}

#[test]
fn build_zed_remote_url_allows_bracketed_ipv6_host() {
    let url = zed_remote::build_zed_remote_url(
        &SshTarget {
            user: "alice".to_string(),
            host: "[::1]".to_string(),
            port: Some(2222),
        },
        "/home/alice/a.py",
    )
    .unwrap();

    assert_eq!(url, "ssh://alice@[::1]:2222/home/alice/a.py");
}

#[test]
fn target_from_payload_splits_codex_managed_authority() {
    let target =
        zed_remote::target_from_payload(&json!({"ssh": {"host": "testuser@10.0.0.1"}})).unwrap();

    assert_eq!(
        target,
        SshTarget {
            user: "testuser".to_string(),
            host: "10.0.0.1".to_string(),
            port: None,
        }
    );
}

#[test]
fn resolve_ssh_target_from_global_state_for_codex_managed_connection() {
    let state = json!({
        "codex-managed-remote-connections": [{
            "hostId": "remote-ssh-codex-managed:remote",
            "displayName": "remote",
            "source": "codex-managed",
            "hostname": "testuser@10.0.0.1",
            "sshPort": null,
        }]
    });

    let target =
        zed_remote::resolve_ssh_target_from_global_state(&state, "remote-ssh-codex-managed:remote")
            .unwrap();

    assert_eq!(
        target,
        SshTarget {
            user: "testuser".to_string(),
            host: "10.0.0.1".to_string(),
            port: None,
        }
    );
}

#[test]
fn fallback_open_request_uses_selected_remote_project() {
    let state = json!({
        "selected-remote-host-id": "remote-ssh-codex-managed:remote",
        "codex-managed-remote-connections": [{
            "hostId": "remote-ssh-codex-managed:remote",
            "hostname": "testuser@10.0.0.1",
            "sshPort": null,
        }],
        "remote-projects": [{
            "id": "032e652b-7956-4e6e-83bd-b29f456c6c3d",
            "hostId": "remote-ssh-codex-managed:remote",
            "remotePath": "/Users/testuser/projects/sample",
            "label": "sample",
        }],
        "project-order": ["032e652b-7956-4e6e-83bd-b29f456c6c3d"],
    });

    let request =
        zed_remote::fallback_open_request_from_global_state_with_context(&state, "", "", "", "")
            .unwrap();

    assert_eq!(
        request,
        json!({
            "hostId": "remote-ssh-codex-managed:remote",
            "ssh": {"user": "testuser", "host": "10.0.0.1", "port": null},
            "path": "/Users/testuser/projects/sample",
        })
    );
}

#[test]
fn fallback_open_request_prefers_project_order_for_selected_host() {
    let state = json!({
        "selected-remote-host-id": "remote-ssh-codex-managed:remote",
        "codex-managed-remote-connections": [{
            "hostId": "remote-ssh-codex-managed:remote",
            "hostname": "testuser@10.0.0.1",
        }],
        "remote-projects": [
            {"id": "old", "hostId": "remote-ssh-codex-managed:remote", "remotePath": "/Users/testuser/projects/old"},
            {"id": "current", "hostId": "remote-ssh-codex-managed:remote", "remotePath": "/Users/testuser/projects/current"},
            {"id": "other-host", "hostId": "remote-ssh-codex-managed:other", "remotePath": "/srv/other"}
        ],
        "project-order": ["other-host", "current", "old"],
    });

    let request =
        zed_remote::fallback_open_request_from_global_state_with_context(&state, "", "", "", "")
            .unwrap();

    assert_eq!(request["hostId"], "remote-ssh-codex-managed:remote");
    assert_eq!(request["path"], "/Users/testuser/projects/current");
}

#[test]
fn fallback_open_request_prefers_remote_project_id_context() {
    let state = json!({
        "selected-remote-host-id": "remote-ssh-codex-managed:remote",
        "codex-managed-remote-connections": [{
            "hostId": "remote-ssh-codex-managed:remote",
            "hostname": "testuser@10.0.0.1",
        }],
        "remote-projects": [
            {
                "id": "032e652b-7956-4e6e-83bd-b29f456c6c3d",
                "hostId": "remote-ssh-codex-managed:remote",
                "remotePath": "/Users/testuser/projects/sample",
            },
            {
                "id": "a21be7c9-a917-433a-bfc7-f422a34c2185",
                "hostId": "remote-ssh-codex-managed:remote",
                "remotePath": "/Users/testuser/projects/sample-b",
            },
        ],
        "project-order": ["032e652b-7956-4e6e-83bd-b29f456c6c3d", "a21be7c9-a917-433a-bfc7-f422a34c2185"],
    });

    let request = zed_remote::fallback_open_request_from_global_state_with_context(
        &state,
        "remote-ssh-codex-managed:remote",
        "",
        "",
        "a21be7c9-a917-433a-bfc7-f422a34c2185",
    )
    .unwrap();

    assert_eq!(request["hostId"], "remote-ssh-codex-managed:remote");
    assert_eq!(request["path"], "/Users/testuser/projects/sample-b");
}

#[test]
fn fallback_open_request_treats_remote_project_id_as_path() {
    let state = json!({
        "selected-remote-host-id": "remote-ssh-codex-managed:remote",
        "codex-managed-remote-connections": [{
            "hostId": "remote-ssh-codex-managed:remote",
            "hostname": "testuser@10.0.0.1",
        }],
        "remote-projects": [{
            "id": "032e652b-7956-4e6e-83bd-b29f456c6c3d",
            "hostId": "remote-ssh-codex-managed:remote",
            "remotePath": "/Users/testuser/projects/sample",
        }],
        "project-order": ["032e652b-7956-4e6e-83bd-b29f456c6c3d"],
    });

    let request = zed_remote::fallback_open_request_from_global_state_with_context(
        &state,
        "remote-ssh-codex-managed:remote",
        "",
        "",
        "/Users/testuser/projects/sample-b",
    )
    .unwrap();

    assert_eq!(request["hostId"], "remote-ssh-codex-managed:remote");
    assert_eq!(request["path"], "/Users/testuser/projects/sample-b");
}

#[test]
fn fallback_open_request_prefers_thread_workspace_hint() {
    let state = json!({
        "selected-remote-host-id": "remote-ssh-codex-managed:remote",
        "codex-managed-remote-connections": [{
            "hostId": "remote-ssh-codex-managed:remote",
            "hostname": "testuser@10.0.0.1",
        }],
        "remote-projects": [{
            "id": "main",
            "hostId": "remote-ssh-codex-managed:remote",
            "remotePath": "/Users/testuser/projects/sample",
        }],
        "project-order": ["main"],
        "thread-workspace-root-hints": {
            "019e39c1-worktree": "/Users/testuser/projects/sample/.worktree/zed-fix",
        },
    });

    let request = zed_remote::fallback_open_request_from_global_state_with_context(
        &state,
        "",
        "019e39c1-worktree",
        "",
        "",
    )
    .unwrap();

    assert_eq!(request["hostId"], "remote-ssh-codex-managed:remote");
    assert_eq!(
        request["path"],
        "/Users/testuser/projects/sample/.worktree/zed-fix"
    );
}

#[test]
fn fallback_open_request_accepts_local_prefixed_thread_workspace_hint() {
    let state = json!({
        "selected-remote-host-id": "remote-ssh-codex-managed:remote",
        "codex-managed-remote-connections": [{
            "hostId": "remote-ssh-codex-managed:remote",
            "hostname": "testuser@10.0.0.1",
        }],
        "remote-projects": [{
            "id": "main",
            "hostId": "remote-ssh-codex-managed:remote",
            "remotePath": "/Users/testuser/projects/sample",
        }],
        "project-order": ["main"],
        "thread-workspace-root-hints": {
            "019e39c1-worktree": "/Users/testuser/projects/sample/.worktree/zed-fix",
        },
    });

    let request = zed_remote::fallback_open_request_from_global_state_with_context(
        &state,
        "",
        "local:019e39c1-worktree",
        "",
        "",
    )
    .unwrap();

    assert_eq!(request["hostId"], "remote-ssh-codex-managed:remote");
    assert_eq!(
        request["path"],
        "/Users/testuser/projects/sample/.worktree/zed-fix"
    );
}

#[test]
fn fallback_open_request_response_passes_thread_workspace_hint() {
    let state = json!({
        "selected-remote-host-id": "remote-ssh-codex-managed:remote",
        "codex-managed-remote-connections": [{
            "hostId": "remote-ssh-codex-managed:remote",
            "hostname": "testuser@10.0.0.1",
        }],
        "remote-projects": [{
            "id": "main",
            "hostId": "remote-ssh-codex-managed:remote",
            "remotePath": "/Users/testuser/projects/sample",
        }],
        "thread-workspace-root-hints": {
            "019e39c1-worktree": "/Users/testuser/projects/sample/.worktree/zed-fix",
        },
    });

    let request = zed_remote::fallback_open_request_from_global_state_with_context(
        &state,
        "",
        "019e39c1-worktree",
        "",
        "",
    )
    .unwrap();

    assert_eq!(
        request["path"],
        "/Users/testuser/projects/sample/.worktree/zed-fix"
    );
}

#[test]
fn workspace_root_from_sqlite_reads_thread_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("state_5.sqlite");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    db.execute(
        "CREATE TABLE threads (id TEXT PRIMARY KEY, cwd TEXT NOT NULL)",
        [],
    )
    .unwrap();
    db.execute(
        "INSERT INTO threads (id, cwd) VALUES (?1, ?2)",
        (
            "019e39c1-worktree",
            "/Users/testuser/projects/sample/.worktree/zed-fix",
        ),
    )
    .unwrap();
    drop(db);

    let cwd = zed_remote::workspace_root_from_sqlite("local:019e39c1-worktree", Some(&db_path));

    assert_eq!(cwd, "/Users/testuser/projects/sample/.worktree/zed-fix");
}

#[test]
fn fallback_open_request_reports_missing_remote_project() {
    let state = json!({"selected-remote-host-id": "remote-ssh-codex-managed:remote"});

    let error =
        zed_remote::fallback_open_request_from_global_state_with_context(&state, "", "", "", "")
            .unwrap_err();

    assert_eq!(
        error.to_string(),
        "Cannot determine remote workspace or file for Zed"
    );
}

#[test]
fn resolve_ssh_target_response_reports_missing_host_id() {
    let result = zed_remote::resolve_ssh_target_response(&json!({"hostId": ""}));

    assert_eq!(
        result,
        json!({"status": "failed", "message": "Remote host id is required"})
    );
}

#[test]
fn open_zed_remote_returns_failed_response_for_validation_error() {
    let result = zed_remote::open_zed_remote(&json!({"ssh": {"host": ""}, "path": "/a.py"}));

    assert_eq!(
        result,
        json!({"status": "failed", "message": "Cannot determine remote SSH host for this file"})
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_candidates_include_applications_zed_app() {
    let candidates = zed_remote::candidate_zed_app_paths();
    let labels: Vec<String> = candidates
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    assert!(
        labels.iter().any(|p| p == "/Applications/Zed.app"),
        "macOS candidates missing /Applications/Zed.app: {labels:?}",
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_candidates_include_localappdata_programs_zed() {
    use std::path::PathBuf;
    // SAFETY: Test runs single-threaded with a unique env var name; no other
    // code in the suite reads LOCALAPPDATA at the same time. set_var on Windows
    // is sound when no other thread is concurrently reading the same key.
    unsafe {
        std::env::set_var("LOCALAPPDATA", r"C:\Users\test\AppData\Local");
    }
    let candidates = zed_remote::candidate_zed_app_paths();
    let expected = PathBuf::from(r"C:\Users\test\AppData\Local\Programs\Zed").join("Zed.exe");
    assert!(
        candidates.contains(&expected),
        "Windows candidates missing {expected:?}: {candidates:?}",
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_candidates_include_program_files_zed() {
    use std::path::PathBuf;
    unsafe {
        std::env::set_var("ProgramFiles", r"C:\Program Files");
    }
    let candidates = zed_remote::candidate_zed_app_paths();
    let expected = PathBuf::from(r"C:\Program Files\Zed").join("Zed.exe");
    assert!(
        candidates.contains(&expected),
        "Windows candidates missing {expected:?}: {candidates:?}",
    );
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn non_mac_non_windows_candidates_are_empty() {
    // Linux relies on the `zed` CLI on PATH; no app-bundle discovery applies.
    let candidates = zed_remote::candidate_zed_app_paths();
    assert!(
        candidates.is_empty(),
        "expected empty candidate list on this platform, got: {candidates:?}",
    );
}
