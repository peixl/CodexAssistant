use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::settings::{BackendSettings, SettingsStore, normalize_codex_extra_args};
use crate::status::{LaunchStatus, StatusStore};

/// Number of times to retry CDP bridge injection while waiting for Codex to open its
/// `--remote-debugging-port` endpoint. Codex Desktop (MSIX) cold-starts can take 30+ seconds
/// before `/json` is reachable, so the budget needs to accommodate that.
pub const BRIDGE_INJECTION_RETRY_COUNT: usize = 120;

/// Interval between bridge injection retries.
pub const BRIDGE_INJECTION_RETRY_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexLaunch {
    Process {
        command: Vec<String>,
        wait_strategy: ProcessWaitStrategy,
        macos_cleanup_policy: Option<MacosCleanupPolicy>,
    },
    PackagedActivation {
        app_user_model_id: String,
        arguments: String,
        process_id: Option<u32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessWaitStrategy {
    TrackedChild,
    ExternalWaitCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacosCleanupPolicy {
    QuitIfNotPreviouslyRunning,
    SkipQuitBecauseAlreadyRunning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsProcessControlStrategy {
    NativeWindowsApi,
}

#[cfg(windows)]
pub fn windows_process_control_strategy() -> WindowsProcessControlStrategy {
    WindowsProcessControlStrategy::NativeWindowsApi
}

impl CodexLaunch {
    pub fn process_id(&self) -> Option<u32> {
        match self {
            Self::PackagedActivation { process_id, .. } => *process_id,
            Self::Process { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub app_dir: Option<PathBuf>,
    pub debug_port: u16,
    pub helper_port: u16,
    pub status_store: StatusStore,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            app_dir: None,
            debug_port: 9229,
            helper_port: 57321,
            status_store: StatusStore::default(),
        }
    }
}

#[derive(Clone)]
pub struct LaunchHandle {
    pub debug_port: u16,
    pub helper_port: u16,
    pub app_dir: PathBuf,
    pub launch: CodexLaunch,
    pub status_store: StatusStore,
    helper_started: bool,
    hooks: Arc<dyn LaunchHooks>,
}

impl std::fmt::Debug for LaunchHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LaunchHandle")
            .field("debug_port", &self.debug_port)
            .field("helper_port", &self.helper_port)
            .field("app_dir", &self.app_dir)
            .field("launch", &self.launch)
            .field("status_store", &self.status_store)
            .finish_non_exhaustive()
    }
}

impl LaunchHandle {
    pub async fn wait_for_codex_exit(&self) -> anyhow::Result<()> {
        let result = self.hooks.wait_for_codex_exit(&self.launch).await;
        if self.helper_started {
            self.hooks.shutdown_helper(self.helper_port).await;
        }
        result
    }
}

#[async_trait(?Send)]
pub trait LaunchHooks: Send + Sync {
    fn resolve_app_dir(
        &self,
        app_dir: Option<&Path>,
        settings: &BackendSettings,
    ) -> anyhow::Result<PathBuf>;
    fn select_debug_port(&self, requested: u16) -> u16;
    fn select_helper_port(&self, requested: u16) -> u16;
    async fn load_settings(&self) -> anyhow::Result<BackendSettings>;
    async fn run_provider_sync(&self) -> anyhow::Result<()>;
    /// Verify that Windows TCP loopback works on this machine before launching Codex.
    /// Default impl runs [`preflight_loopback_reachable`]. Test hooks override this
    /// to avoid depending on the test host's networking.
    async fn verify_loopback_reachable(&self) -> anyhow::Result<()> {
        preflight_loopback_reachable().await
    }
    async fn start_helper(&self, helper_port: u16) -> anyhow::Result<()>;
    async fn launch_codex(
        &self,
        app_dir: &Path,
        debug_port: u16,
        extra_args: &[String],
    ) -> anyhow::Result<CodexLaunch>;
    async fn bridge_context(
        &self,
        _debug_port: u16,
    ) -> anyhow::Result<Option<crate::routes::BridgeContext>> {
        Ok(None)
    }
    async fn inject(&self, debug_port: u16, helper_port: u16) -> anyhow::Result<()>;
    async fn inject_bridge(
        &self,
        debug_port: u16,
        helper_port: u16,
        _ctx: crate::routes::BridgeContext,
    ) -> anyhow::Result<()> {
        self.inject(debug_port, helper_port).await
    }
    async fn start_bridge_watchdog(
        &self,
        _debug_port: u16,
        _helper_port: u16,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn write_status(&self, status: &str);
    async fn wait_for_codex_exit(&self, launch: &CodexLaunch) -> anyhow::Result<()>;
    async fn shutdown_helper(&self, helper_port: u16);
    async fn terminate_codex(&self, launch: &CodexLaunch);
}

#[derive(Default)]
pub struct DefaultLaunchHooks {
    child: Mutex<Option<Child>>,
    helper: Mutex<Option<HelperRuntime>>,
    bridge_watchdog: Mutex<Option<BridgeWatchdogRuntime>>,
}

struct HelperRuntime {
    shutdown: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

struct BridgeWatchdogRuntime {
    shutdown: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

pub async fn launch_and_inject(options: LaunchOptions) -> anyhow::Result<LaunchHandle> {
    launch_and_inject_with_hooks(options, DefaultLaunchHooks::shared()).await
}

pub async fn launch_and_inject_with_hooks<H>(
    options: LaunchOptions,
    hooks: H,
) -> anyhow::Result<LaunchHandle>
where
    H: IntoLaunchHooks,
{
    let hooks = hooks.into_launch_hooks();
    let debug_port = hooks.select_debug_port(options.debug_port);
    let mut helper_port = hooks.select_helper_port(options.helper_port);
    let status_store = options.status_store.clone();

    let settings = match hooks.load_settings().await {
        Ok(settings) => settings,
        Err(error) => {
            return Err(record_early_failure(
                &status_store,
                &hooks,
                debug_port,
                helper_port,
                Path::new(""),
                error,
                "load_settings",
            )
            .await);
        }
    };
    let app_dir = match hooks.resolve_app_dir(options.app_dir.as_deref(), &settings) {
        Ok(app_dir) => app_dir,
        Err(error) => {
            return Err(record_early_failure(
                &status_store,
                &hooks,
                debug_port,
                helper_port,
                Path::new(""),
                error,
                "resolve_app_dir",
            )
            .await);
        }
    };
    let mut helper_started = false;
    let mut launched = None;

    let result: anyhow::Result<LaunchHandle> = async {
        if settings.enhancements_enabled {
            hooks.verify_loopback_reachable().await?;
        }

        if settings.provider_sync_enabled {
            hooks.run_provider_sync().await?;
        }

        let protocol_proxy_enabled = relay_protocol_proxy_enabled(&settings);
        if protocol_proxy_enabled {
            helper_port = crate::protocol_proxy::DEFAULT_PROTOCOL_PROXY_PORT;
        }
        if settings.enhancements_enabled || protocol_proxy_enabled {
            hooks.start_helper(helper_port).await?;
            helper_started = true;
        }

        let launch = hooks
            .launch_codex(&app_dir, debug_port, &settings.codex_extra_args)
            .await?;
        launched = Some(launch.clone());

        let mut degraded_reason: Option<String> = None;
        if settings.enhancements_enabled {
            let bridge_context_result = hooks.bridge_context(debug_port).await;
            let inject_outcome = match bridge_context_result {
                Ok(Some(ctx)) => hooks.inject_bridge(debug_port, helper_port, ctx).await,
                Ok(None) => hooks.inject(debug_port, helper_port).await,
                Err(error) => Err(error),
            };
            match inject_outcome {
                Ok(()) => {
                    if let Err(error) =
                        hooks.start_bridge_watchdog(debug_port, helper_port).await
                    {
                        degraded_reason = Some(format!(
                            "Codex launched but bridge watchdog could not start: {error}. Enhancements may stop working if Codex reloads."
                        ));
                    }
                }
                Err(error) => {
                    degraded_reason = Some(injection_failure_message(&error));
                    let _ = crate::diagnostic_log::append_diagnostic_log(
                        "bridge.injection_failed_keeping_codex_running",
                        serde_json::json!({
                            "debug_port": debug_port,
                            "helper_port": helper_port,
                            "platform": std::env::consts::OS,
                            "message": error.to_string(),
                        }),
                    );
                }
            }
        }

        let (status_label, status_message) = match degraded_reason.as_deref() {
            Some(reason) => ("running_degraded", reason),
            None => ("running", "CodexAssistant launcher ready"),
        };
        let status = launch_status(
            status_label,
            status_message,
            debug_port,
            helper_port,
            &app_dir,
        );
        options.status_store.save_latest(&status)?;
        hooks.write_status(status_label).await;

        Ok(LaunchHandle {
            debug_port,
            helper_port,
            app_dir: app_dir.clone(),
            launch,
            status_store: status_store.clone(),
            helper_started,
            hooks: Arc::clone(&hooks),
        })
    }
    .await;

    match result {
        Ok(handle) => Ok(handle),
        Err(error) => {
            if helper_started {
                hooks.shutdown_helper(helper_port).await;
            }
            if let Some(launch) = &launched {
                hooks.terminate_codex(launch).await;
            }
            let message = error.to_string();
            let failure = launch_status("failed", &message, debug_port, helper_port, &app_dir);
            let _ = status_store.save_latest(&failure);
            hooks.write_status("failed").await;
            Err(error)
        }
    }
}

fn relay_protocol_proxy_enabled(settings: &BackendSettings) -> bool {
    settings.active_relay_profile().protocol == crate::settings::RelayProtocol::ChatCompletions
}

pub trait IntoLaunchHooks {
    fn into_launch_hooks(self) -> Arc<dyn LaunchHooks>;
}

impl<T> IntoLaunchHooks for &T
where
    T: LaunchHooks + Clone + 'static,
{
    fn into_launch_hooks(self) -> Arc<dyn LaunchHooks> {
        Arc::new(self.clone())
    }
}

impl IntoLaunchHooks for Arc<dyn LaunchHooks> {
    fn into_launch_hooks(self) -> Arc<dyn LaunchHooks> {
        self
    }
}

impl IntoLaunchHooks for DefaultLaunchHooks {
    fn into_launch_hooks(self) -> Arc<dyn LaunchHooks> {
        Arc::new(self)
    }
}

impl DefaultLaunchHooks {
    pub fn shared() -> Arc<dyn LaunchHooks> {
        Arc::new(Self::default())
    }
}

#[async_trait(?Send)]
impl LaunchHooks for DefaultLaunchHooks {
    fn resolve_app_dir(
        &self,
        app_dir: Option<&Path>,
        settings: &BackendSettings,
    ) -> anyhow::Result<PathBuf> {
        crate::app_paths::resolve_codex_app_dir_with_saved(
            app_dir,
            Some(settings.codex_app_path.as_str()),
        )
        .ok_or_else(|| anyhow::anyhow!("Codex App directory not found"))
    }

    fn select_debug_port(&self, requested: u16) -> u16 {
        crate::ports::select_platform_loopback_port(requested)
    }

    fn select_helper_port(&self, requested: u16) -> u16 {
        crate::ports::select_platform_loopback_port(requested)
    }

    async fn load_settings(&self) -> anyhow::Result<BackendSettings> {
        SettingsStore::default().load()
    }

    async fn run_provider_sync(&self) -> anyhow::Result<()> {
        anyhow::bail!("provider sync requires launcher hooks with codex-assistant-data integration")
    }

    async fn start_helper(&self, helper_port: u16) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", helper_port))
            .await
            .with_context(|| format!("failed to bind helper runtime on 127.0.0.1:{helper_port}"))?;
        let _ = crate::diagnostic_log::append_diagnostic_log(
            "helper.listening",
            serde_json::json!({
                "helper_port": helper_port,
                "address": format!("http://127.0.0.1:{helper_port}")
            }),
        );
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accepted = listener.accept() => {
                        if let Ok((stream, addr)) = accepted {
                            tokio::spawn(async move {
                                let _ = handle_helper_connection(stream, Some(addr)).await;
                            });
                        }
                    }
                }
            }
        });
        *self.helper.lock().await = Some(HelperRuntime {
            shutdown: shutdown_tx,
            task,
        });
        Ok(())
    }

    async fn launch_codex(
        &self,
        app_dir: &Path,
        debug_port: u16,
        extra_args: &[String],
    ) -> anyhow::Result<CodexLaunch> {
        if cfg!(windows)
            && let Some(activation) = build_packaged_activation(app_dir, debug_port, extra_args)
        {
            let CodexLaunch::PackagedActivation {
                app_user_model_id,
                arguments,
                ..
            } = &activation
            else {
                unreachable!();
            };
            let env = codex_process_environment();
            let process_id =
                activate_packaged_app_with_environment(app_user_model_id, arguments, &env).await?;
            return Ok(match activation {
                CodexLaunch::PackagedActivation {
                    app_user_model_id,
                    arguments,
                    ..
                } => CodexLaunch::PackagedActivation {
                    app_user_model_id,
                    arguments,
                    process_id: Some(process_id),
                },
                CodexLaunch::Process { .. } => unreachable!(),
            });
        }

        if app_dir.extension().and_then(|value| value.to_str()) == Some("app") {
            let cleanup_policy = if is_macos_app_running(app_dir).await {
                MacosCleanupPolicy::SkipQuitBecauseAlreadyRunning
            } else {
                MacosCleanupPolicy::QuitIfNotPreviouslyRunning
            };
            let command = build_macos_open_command(app_dir, debug_port, extra_args);
            let executable = command
                .first()
                .ok_or_else(|| anyhow::anyhow!("macOS open command is empty"))?;
            let child = Command::new(executable)
                .args(&command[1..])
                .envs(codex_process_environment())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("failed to launch macOS Codex app")?;
            *self.child.lock().await = Some(child);
            return Ok(CodexLaunch::Process {
                command,
                wait_strategy: ProcessWaitStrategy::ExternalWaitCommand,
                macos_cleanup_policy: Some(cleanup_policy),
            });
        }

        let command = build_codex_command(app_dir, debug_port, extra_args);
        let executable = command
            .first()
            .ok_or_else(|| anyhow::anyhow!("Codex command is empty"))?;
        let mut child_command = Command::new(executable);
        child_command
            .args(&command[1..])
            .envs(codex_process_environment())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        child_command.creation_flags(crate::windows_integration::CREATE_NO_WINDOW);
        let child = child_command
            .spawn()
            .with_context(|| format!("failed to launch Codex executable {executable}"))?;
        *self.child.lock().await = Some(child);
        Ok(CodexLaunch::Process {
            command,
            wait_strategy: ProcessWaitStrategy::TrackedChild,
            macos_cleanup_policy: None,
        })
    }

    async fn inject(&self, debug_port: u16, helper_port: u16) -> anyhow::Result<()> {
        retry_injection(debug_port, helper_port).await
    }

    async fn start_bridge_watchdog(&self, debug_port: u16, helper_port: u16) -> anyhow::Result<()> {
        let (shutdown, mut shutdown_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    _ = interval.tick() => {
                        let _ = check_and_reinject_bridge(debug_port, helper_port).await;
                    }
                }
            }
        });
        let previous = self
            .bridge_watchdog
            .lock()
            .await
            .replace(BridgeWatchdogRuntime { shutdown, task });
        if let Some(runtime) = previous {
            let _ = runtime.shutdown.send(());
            let _ = runtime.task.await;
        }
        Ok(())
    }

    async fn write_status(&self, _status: &str) {}

    async fn wait_for_codex_exit(&self, launch: &CodexLaunch) -> anyhow::Result<()> {
        match launch {
            CodexLaunch::Process { .. } => {
                let child_opt = self.child.lock().await.take();
                if let Some(mut child) = child_opt {
                    let _ = child.wait().await;
                }
                Ok(())
            }
            CodexLaunch::PackagedActivation { process_id, .. } => {
                if let Some(process_id) = process_id {
                    wait_for_windows_process_id(*process_id).await?;
                }
                Ok(())
            }
        }
    }

    async fn shutdown_helper(&self, _helper_port: u16) {
        let bridge = self.bridge_watchdog.lock().await.take();
        if let Some(runtime) = bridge {
            let _ = runtime.shutdown.send(());
            let _ = runtime.task.await;
        }
        let helper = self.helper.lock().await.take();
        if let Some(runtime) = helper {
            let _ = runtime.shutdown.send(());
            let _ = runtime.task.await;
        }
    }

    async fn terminate_codex(&self, launch: &CodexLaunch) {
        match launch {
            CodexLaunch::Process {
                wait_strategy: ProcessWaitStrategy::ExternalWaitCommand,
                command,
                macos_cleanup_policy,
            } => {
                let child_opt = self.child.lock().await.take();
                if let Some(mut child) = child_opt {
                    let _ = child.kill().await;
                }
                if let (Some(app_dir), Some(cleanup_policy)) = (
                    macos_app_dir_from_open_command(command),
                    *macos_cleanup_policy,
                ) {
                    let _ = run_macos_cleanup_command(&app_dir, cleanup_policy).await;
                }
            }
            CodexLaunch::Process { .. } => {
                let child_opt = self.child.lock().await.take();
                if let Some(mut child) = child_opt {
                    let _ = child.kill().await;
                }
            }
            CodexLaunch::PackagedActivation {
                process_id: Some(process_id),
                ..
            } => {
                let _ = terminate_windows_process_id(*process_id).await;
            }
            CodexLaunch::PackagedActivation {
                process_id: None, ..
            } => {}
        }
    }
}

async fn handle_helper_connection(
    mut stream: tokio::net::TcpStream,
    remote_addr: Option<SocketAddr>,
) -> anyhow::Result<()> {
    let request_bytes = read_http_request(&mut stream).await?;
    let request = String::from_utf8_lossy(&request_bytes);
    let request_line = request.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let request_body = http_request_body(&request);
    let remote_addr_text = remote_addr.map(|addr| addr.to_string());

    let _ = crate::diagnostic_log::append_diagnostic_log(
        "helper.request",
        serde_json::json!({
            "method": method,
            "path": path,
            "request_line": request_line,
            "remote_addr": remote_addr_text,
            "body_bytes": request_body.len()
        }),
    );

    if method != "OPTIONS" {
        let provided = extract_helper_token_header(&request);
        if !crate::helper_auth::verify_token(provided.unwrap_or("")) {
            let body = serde_json::to_vec(&serde_json::json!({
                "status": "failed",
                "message": "unauthorized"
            }))?;
            let response = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).await?;
            stream.write_all(&body).await?;
            stream.shutdown().await?;
            let _ = crate::diagnostic_log::append_diagnostic_log(
                "security.helper_token_invalid",
                serde_json::json!({
                    "method": method,
                    "path": path,
                    "provided_len": provided.map(str::len).unwrap_or(0),
                    "remote_addr": remote_addr_text
                }),
            );
            return Ok(());
        }
    }

    if crate::protocol_proxy::is_responses_proxy_path(path) && method == "POST" {
        return handle_protocol_proxy_connection(
            &mut stream,
            request_body,
            method,
            path,
            remote_addr_text,
        )
        .await;
    }
    if crate::protocol_proxy::is_models_proxy_path(path) && matches!(method, "GET" | "OPTIONS") {
        return handle_models_proxy_connection(&mut stream, method, path, remote_addr_text).await;
    }

    let (status, body, content_type, log_event) =
        if matches!(path, "/backend/status" | "/backend/repair")
            && matches!(method, "GET" | "POST" | "OPTIONS")
        {
            (
                "200 OK".to_string(),
                serde_json::to_vec(&serde_json::json!({
                    "status": "ok",
                    "message": "后端已连接",
                    "version": crate::version::VERSION,
                    "transport": "http-helper"
                }))?,
                "application/json; charset=utf-8".to_string(),
                if path == "/backend/status" {
                    "helper.backend_status_ok"
                } else {
                    "helper.backend_repair_ok"
                },
            )
        } else if path == "/diagnostics/log" && matches!(method, "POST" | "OPTIONS") {
            if method == "POST" {
                let detail = serde_json::from_str::<serde_json::Value>(request_body)
                    .unwrap_or_else(|error| {
                        serde_json::json!({
                            "parse_error": error.to_string(),
                            "raw": request_body
                        })
                    });
                let event = detail
                    .get("event")
                    .and_then(serde_json::Value::as_str)
                    .map(sanitize_diagnostic_event)
                    .unwrap_or_else(|| "event".to_string());
                let _ = crate::diagnostic_log::append_diagnostic_log(
                    &format!("renderer.{event}"),
                    detail,
                );
            }
            (
                "200 OK".to_string(),
                serde_json::to_vec(&serde_json::json!({
                    "status": "ok",
                    "message": "日志已记录"
                }))?,
                "application/json; charset=utf-8".to_string(),
                "helper.diagnostics_log_ok",
            )
        } else {
            (
                "404 Not Found".to_string(),
                serde_json::to_vec(&serde_json::json!({
                    "status": "failed",
                    "message": "未知后端路径"
                }))?,
                "application/json; charset=utf-8".to_string(),
                "helper.unknown_path",
            )
        };
    let _ = crate::diagnostic_log::append_diagnostic_log(
        log_event,
        serde_json::json!({
            "method": method,
            "path": path,
            "status": status,
            "remote_addr": remote_addr_text
        }),
    );
    let response = if method == "OPTIONS" {
        "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
    } else {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
    };
    stream.write_all(response.as_bytes()).await?;
    if method != "OPTIONS" {
        stream.write_all(&body).await?;
    }
    stream.shutdown().await?;
    Ok(())
}

async fn handle_models_proxy_connection(
    stream: &mut tokio::net::TcpStream,
    method: &str,
    path: &str,
    remote_addr_text: Option<String>,
) -> anyhow::Result<()> {
    if method == "OPTIONS" {
        write_http_response(
            stream,
            "204 No Content",
            "application/json; charset=utf-8",
            &[],
        )
        .await?;
        stream.shutdown().await?;
        return Ok(());
    }

    let upstream = match crate::protocol_proxy::open_models_proxy_request().await {
        Ok(upstream) => upstream,
        Err(error) => {
            let body = serde_json::to_vec(&serde_json::json!({
                "status": "failed",
                "message": error.to_string()
            }))?;
            write_http_response(
                stream,
                "502 Bad Gateway",
                "application/json; charset=utf-8",
                &body,
            )
            .await?;
            log_helper_response(
                "helper.models_proxy_failed",
                method,
                path,
                "502 Bad Gateway",
                remote_addr_text,
            );
            stream.shutdown().await?;
            return Ok(());
        }
    };

    let status = upstream.status();
    let is_success = upstream.is_success();
    let content_type = if upstream.content_type.is_empty() {
        "application/json; charset=utf-8".to_string()
    } else {
        upstream.content_type.clone()
    };
    let body = upstream.response.bytes().await?.to_vec();
    write_http_response(stream, &status, &content_type, &body).await?;
    log_helper_response(
        if is_success {
            "helper.models_proxy_ok"
        } else {
            "helper.models_proxy_upstream_error"
        },
        method,
        path,
        &status,
        remote_addr_text,
    );
    stream.shutdown().await?;
    Ok(())
}

async fn handle_protocol_proxy_connection(
    stream: &mut tokio::net::TcpStream,
    request_body: &str,
    method: &str,
    path: &str,
    remote_addr_text: Option<String>,
) -> anyhow::Result<()> {
    let upstream = match crate::protocol_proxy::open_responses_proxy_request(request_body).await {
        Ok(upstream) => upstream,
        Err(error) => {
            let body = serde_json::to_vec(&serde_json::json!({
                "status": "failed",
                "message": error.to_string()
            }))?;
            write_http_response(
                stream,
                "502 Bad Gateway",
                "application/json; charset=utf-8",
                &body,
            )
            .await?;
            log_helper_response(
                "helper.protocol_proxy_failed",
                method,
                path,
                "502 Bad Gateway",
                remote_addr_text,
            );
            stream.shutdown().await?;
            return Ok(());
        }
    };

    if !upstream.is_success() {
        let status = upstream.status();
        let content_type = if upstream.content_type.is_empty() {
            "application/json; charset=utf-8".to_string()
        } else {
            upstream.content_type.clone()
        };
        let body = upstream.response.bytes().await?.to_vec();
        write_http_response(stream, &status, &content_type, &body).await?;
        log_helper_response(
            "helper.protocol_proxy_upstream_error",
            method,
            path,
            &status,
            remote_addr_text,
        );
        stream.shutdown().await?;
        return Ok(());
    }

    if upstream.is_stream {
        write_http_stream_headers(stream, "200 OK", "text/event-stream; charset=utf-8").await?;
        let mut converter = crate::protocol_proxy::ChatSseToResponsesConverter::default();
        let mut bytes_stream = upstream.response.bytes_stream();
        let mut stream_failed = false;
        const SSE_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

        loop {
            let next = tokio::time::timeout(SSE_IDLE_TIMEOUT, bytes_stream.next()).await;
            let chunk = match next {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => {
                    let failed = converter.fail(
                        "Upstream SSE idle timeout".to_string(),
                        Some("idle_timeout".to_string()),
                    );
                    if !failed.is_empty() {
                        let _ = stream.write_all(&failed).await;
                    }
                    stream_failed = true;
                    break;
                }
            };
            match chunk {
                Ok(bytes) => {
                    let converted = converter.push_bytes(&bytes);
                    if !converted.is_empty() {
                        stream.write_all(&converted).await?;
                    }
                }
                Err(error) => {
                    let failed = converter.fail(
                        format!("Stream error: {error}"),
                        Some("stream_error".to_string()),
                    );
                    if !failed.is_empty() {
                        stream.write_all(&failed).await?;
                    }
                    stream_failed = true;
                    break;
                }
            }
        }

        if !stream_failed {
            let tail = converter.finish();
            if !tail.is_empty() {
                stream.write_all(&tail).await?;
            }
        }
        log_helper_response(
            "helper.protocol_proxy_stream_ok",
            method,
            path,
            "200 OK",
            remote_addr_text,
        );
        stream.shutdown().await?;
        return Ok(());
    }

    let upstream_body = upstream.response.bytes().await?;
    let chat_json: serde_json::Value = serde_json::from_slice(&upstream_body)?;
    let response_json = crate::protocol_proxy::chat_completion_to_response(chat_json)?;
    let body = serde_json::to_vec(&response_json)?;
    write_http_response(stream, "200 OK", "application/json; charset=utf-8", &body).await?;
    log_helper_response(
        "helper.protocol_proxy_ok",
        method,
        path,
        "200 OK",
        remote_addr_text,
    );
    stream.shutdown().await?;
    Ok(())
}

async fn write_http_response(
    stream: &mut tokio::net::TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> anyhow::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.write_all(body).await?;
    Ok(())
}

async fn write_http_stream_headers(
    stream: &mut tokio::net::TcpStream,
    status: &str,
    content_type: &str,
) -> anyhow::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

fn log_helper_response(
    event: &str,
    method: &str,
    path: &str,
    status: &str,
    remote_addr_text: Option<String>,
) {
    let _ = crate::diagnostic_log::append_diagnostic_log(
        event,
        serde_json::json!({
            "method": method,
            "path": path,
            "status": status,
            "remote_addr": remote_addr_text
        }),
    );
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> anyhow::Result<Vec<u8>> {
    const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
    let mut buffer = Vec::new();
    let mut chunk = vec![0_u8; 4096];
    let mut header_end = None;
    let mut content_length = 0_usize;

    loop {
        let read = tokio::time::timeout(READ_TIMEOUT, stream.read(&mut chunk))
            .await
            .map_err(|_| anyhow::anyhow!("HTTP 请求读取超时"))??;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if header_end.is_none() {
            header_end = find_header_end(&buffer);
            if let Some(end) = header_end {
                content_length = content_length_from_headers(&buffer[..end]).unwrap_or(0);
            }
        }
        if let Some(end) = header_end
            && buffer.len() >= end + 4 + content_length
        {
            break;
        }
        if buffer.len() > 32 * 1024 * 1024 {
            anyhow::bail!("HTTP 请求过大");
        }
    }

    Ok(buffer)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn content_length_from_headers(headers: &[u8]) -> Option<usize> {
    let text = String::from_utf8_lossy(headers);
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-length") {
            value.trim().parse().ok()
        } else {
            None
        }
    })
}

fn http_request_body(request: &str) -> &str {
    request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default()
}

fn extract_helper_token_header(request: &str) -> Option<&str> {
    let headers = request
        .split_once("\r\n\r\n")
        .map(|(h, _)| h)
        .unwrap_or(request);
    for line in headers.lines() {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("x-codex-helper-token")
        {
            return Some(value.trim());
        }
    }
    None
}

fn sanitize_diagnostic_event(event: &str) -> String {
    let sanitized = event
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "event".to_string()
    } else {
        sanitized
    }
}

pub fn build_codex_arguments(debug_port: u16, extra_args: &[String]) -> Vec<String> {
    let mut args = vec![
        format!("--remote-debugging-port={debug_port}"),
        format!("--remote-allow-origins=http://127.0.0.1:{debug_port}"),
    ];
    args.extend(normalize_codex_extra_args(extra_args));
    args
}

pub fn build_codex_command(app_dir: &Path, debug_port: u16, extra_args: &[String]) -> Vec<String> {
    let mut command = vec![
        crate::app_paths::build_codex_executable(app_dir)
            .to_string_lossy()
            .to_string(),
    ];
    command.extend(build_codex_arguments(debug_port, extra_args));
    command
}

pub fn build_packaged_activation(
    app_dir: &Path,
    debug_port: u16,
    extra_args: &[String],
) -> Option<CodexLaunch> {
    Some(CodexLaunch::PackagedActivation {
        app_user_model_id: crate::app_paths::packaged_app_user_model_id(app_dir)?,
        arguments: command_line_arguments(&build_codex_arguments(debug_port, extra_args)),
        process_id: None,
    })
}

pub fn codex_process_environment() -> HashMap<String, String> {
    let env = std::env::vars().collect::<HashMap<_, _>>();
    codex_process_environment_from(&env, crate::proxy::detect_system_proxy)
}

pub fn codex_process_environment_from(
    env: &HashMap<String, String>,
    detect_system_proxy: impl FnOnce() -> Option<String>,
) -> HashMap<String, String> {
    let mut env = env.clone();
    mirror_proxy_environment_keys(&mut env);
    ensure_loopback_no_proxy(&mut env);
    if crate::proxy::has_proxy_environment(&env) {
        return env;
    }
    if let Some(proxy) = detect_system_proxy() {
        env.entry("HTTP_PROXY".to_string())
            .or_insert_with(|| proxy.clone());
        env.entry("HTTPS_PROXY".to_string())
            .or_insert_with(|| proxy.clone());
        env.entry("ALL_PROXY".to_string()).or_insert(proxy);
        mirror_proxy_environment_keys(&mut env);
        ensure_loopback_no_proxy(&mut env);
    }
    env
}

fn mirror_proxy_environment_keys(env: &mut HashMap<String, String>) {
    for (upper, lower) in [
        ("HTTP_PROXY", "http_proxy"),
        ("HTTPS_PROXY", "https_proxy"),
        ("ALL_PROXY", "all_proxy"),
    ] {
        match (env.get(upper).cloned(), env.get(lower).cloned()) {
            (Some(value), None) if !value.trim().is_empty() => {
                env.insert(lower.to_string(), value);
            }
            (None, Some(value)) if !value.trim().is_empty() => {
                env.insert(upper.to_string(), value);
            }
            _ => {}
        }
    }
}

fn ensure_loopback_no_proxy(env: &mut HashMap<String, String>) {
    const REQUIRED: [&str; 4] = ["127.0.0.1", "localhost", "::1", "0.0.0.0"];
    let existing_key = ["NO_PROXY", "no_proxy"]
        .into_iter()
        .find(|key| env.get(*key).is_some_and(|value| !value.trim().is_empty()));
    let mut values = existing_key
        .and_then(|key| env.get(key).cloned())
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    for required in REQUIRED {
        if !values
            .iter()
            .any(|value| value.eq_ignore_ascii_case(required))
        {
            values.push(required.to_string());
        }
    }
    let joined = values.join(",");
    env.insert("NO_PROXY".to_string(), joined.clone());
    env.insert("no_proxy".to_string(), joined);
}

async fn retry_injection(debug_port: u16, helper_port: u16) -> anyhow::Result<()> {
    let mut last_error = None;
    for _ in 0..BRIDGE_INJECTION_RETRY_COUNT {
        match try_inject(debug_port, helper_port).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(BRIDGE_INJECTION_RETRY_INTERVAL).await;
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Codex injection failed")))
}

pub async fn check_and_reinject_bridge(debug_port: u16, helper_port: u16) -> bool {
    let healthy = match bridge_health_ok(debug_port).await {
        Ok(healthy) => healthy,
        Err(error) => {
            let _ = crate::diagnostic_log::append_diagnostic_log(
                "bridge.health_check_failed",
                serde_json::json!({
                    "debug_port": debug_port,
                    "helper_port": helper_port,
                    "message": error.to_string()
                }),
            );
            false
        }
    };
    if healthy {
        return false;
    }

    let _ = crate::diagnostic_log::append_diagnostic_log(
        "bridge.reinject_start",
        serde_json::json!({
            "debug_port": debug_port,
            "helper_port": helper_port
        }),
    );
    match retry_injection(debug_port, helper_port).await {
        Ok(()) => {
            let _ = crate::diagnostic_log::append_diagnostic_log(
                "bridge.reinject_ok",
                serde_json::json!({
                    "debug_port": debug_port,
                    "helper_port": helper_port
                }),
            );
            true
        }
        Err(error) => {
            let _ = crate::diagnostic_log::append_diagnostic_log(
                "bridge.reinject_failed",
                serde_json::json!({
                    "debug_port": debug_port,
                    "helper_port": helper_port,
                    "message": error.to_string()
                }),
            );
            false
        }
    }
}

async fn bridge_health_ok(debug_port: u16) -> anyhow::Result<bool> {
    let targets = crate::cdp::list_targets(debug_port).await?;
    let target = crate::cdp::pick_page_target(&targets)?;
    let websocket_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("selected CDP target has no websocket URL"))?;
    let result = crate::bridge::evaluate_script_with_await_promise(
        websocket_url,
        crate::bridge::bridge_health_check_script(),
        true,
    )
    .await?;
    Ok(runtime_evaluate_result_is_true(&result))
}

fn runtime_evaluate_result_is_true(result: &Value) -> bool {
    result
        .get("result")
        .and_then(|result| result.get("result"))
        .and_then(|result| result.get("value"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

async fn try_inject(debug_port: u16, helper_port: u16) -> anyhow::Result<()> {
    let targets = crate::cdp::list_targets(debug_port).await?;
    let target = crate::cdp::pick_page_target(&targets)?;
    let websocket_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("selected CDP target has no websocket URL"))?;
    let script =
        crate::assets::injection_script(helper_port, crate::helper_auth::ensure_helper_token());
    let ctx = crate::routes::BridgeContext::core(Arc::new(crate::routes::CoreRuntimeService::new(
        debug_port,
        StatusStore::default(),
    )));
    crate::bridge::install_bridge(
        websocket_url,
        crate::bridge::BRIDGE_BINDING_NAME,
        Arc::new(move |path, payload| {
            let ctx = ctx.clone();
            Box::pin(
                async move { Ok(crate::routes::handle_bridge_request(ctx, &path, payload).await) },
            )
        }),
        &[script],
    )
    .await
}

pub fn build_macos_open_command(
    app_dir: &Path,
    debug_port: u16,
    extra_args: &[String],
) -> Vec<String> {
    let mut command = vec![
        "open".to_string(),
        "-W".to_string(),
        "-a".to_string(),
        app_dir.to_string_lossy().to_string(),
        "--args".to_string(),
    ];
    command.extend(build_codex_arguments(debug_port, extra_args));
    command
}

pub fn build_macos_cleanup_command(
    app_dir: &Path,
    policy: MacosCleanupPolicy,
) -> Option<Vec<String>> {
    if policy == MacosCleanupPolicy::SkipQuitBecauseAlreadyRunning {
        return None;
    }
    let app_name = app_dir
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Codex");
    Some(vec![
        "osascript".to_string(),
        "-e".to_string(),
        format!(
            r#"tell application "{}" to quit"#,
            app_name.replace('"', "\\\"")
        ),
    ])
}

async fn run_macos_cleanup_command(
    app_dir: &Path,
    policy: MacosCleanupPolicy,
) -> anyhow::Result<()> {
    let Some(command) = build_macos_cleanup_command(app_dir, policy) else {
        return Ok(());
    };
    let Some(executable) = command.first() else {
        return Ok(());
    };
    let _ = Command::new(executable)
        .args(&command[1..])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .with_context(|| format!("failed to request macOS app quit for {}", app_dir.display()))?;
    Ok(())
}

fn macos_app_dir_from_open_command(command: &[String]) -> Option<PathBuf> {
    let app_index = command.iter().position(|part| part == "-a")?;
    command.get(app_index + 1).map(PathBuf::from)
}

async fn is_macos_app_running(app_dir: &Path) -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    let app_name = app_dir
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Codex");
    let script = format!(
        r#"application "{}" is running"#,
        app_name.replace('"', "\\\"")
    );
    let Ok(output) = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    else {
        return false;
    };
    output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .trim()
            .eq_ignore_ascii_case("true")
}

pub fn with_temporary_proxy_environment<T>(
    env: &HashMap<String, String>,
    run: impl FnOnce() -> T,
) -> T {
    let previous = apply_proxy_environment(env);
    let result = run();
    restore_proxy_environment(previous);
    result
}

async fn activate_packaged_app_with_environment(
    app_user_model_id: &str,
    arguments: &str,
    env: &HashMap<String, String>,
) -> anyhow::Result<u32> {
    let previous = apply_proxy_environment(env);
    let result = activate_packaged_app(app_user_model_id, arguments).await;
    restore_proxy_environment(previous);
    result
}

fn apply_proxy_environment(
    env: &HashMap<String, String>,
) -> [(&'static str, Option<std::ffi::OsString>); 8] {
    let keys = [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
    ];
    let previous = keys.map(|key| (key, std::env::var_os(key)));
    for key in keys {
        if let Some(value) = env.get(key) {
            set_env_var(key, value);
        }
    }
    previous
}

fn restore_proxy_environment(previous: [(&'static str, Option<std::ffi::OsString>); 8]) {
    for (key, value) in previous {
        match value {
            Some(value) => set_env_var(key, value),
            None => remove_env_var(key),
        }
    }
}

#[cfg(windows)]
async fn wait_for_windows_process_id(process_id: u32) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || wait_for_windows_process_id_blocking(process_id))
        .await
        .context("Windows process wait task failed")?
}

#[cfg(windows)]
async fn terminate_windows_process_id(process_id: u32) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || terminate_windows_process_id_blocking(process_id))
        .await
        .context("Windows process termination task failed")?
}

#[cfg(windows)]
fn wait_for_windows_process_id_blocking(process_id: u32) -> anyhow::Result<()> {
    use windows::Win32::Foundation::{CloseHandle, WAIT_FAILED};
    use windows::Win32::System::Threading::{
        INFINITE, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        WaitForSingleObject,
    };

    unsafe {
        let handle = OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            process_id,
        )
        .with_context(|| format!("failed to open Windows process id {process_id}"))?;
        let wait_result = WaitForSingleObject(handle, INFINITE);
        let _ = CloseHandle(handle);
        if wait_result == WAIT_FAILED {
            anyhow::bail!("failed to wait for Windows process id {process_id}");
        }
    }
    Ok(())
}

#[cfg(windows)]
fn terminate_windows_process_id_blocking(process_id: u32) -> anyhow::Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, TerminateProcess,
    };

    unsafe {
        let handle = OpenProcess(
            PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            process_id,
        )
        .with_context(|| format!("failed to open Windows process id {process_id}"))?;
        let terminate_result = TerminateProcess(handle, 1);
        let _ = CloseHandle(handle);
        terminate_result
            .with_context(|| format!("failed to terminate Windows process id {process_id}"))?;
    }
    Ok(())
}

#[cfg(not(windows))]
async fn wait_for_windows_process_id(process_id: u32) -> anyhow::Result<()> {
    anyhow::bail!("cannot wait for Windows process id {process_id} on this platform")
}

#[cfg(not(windows))]
async fn terminate_windows_process_id(process_id: u32) -> anyhow::Result<()> {
    anyhow::bail!("cannot terminate Windows process id {process_id} on this platform")
}

fn set_env_var<K, V>(key: K, value: V)
where
    K: AsRef<std::ffi::OsStr>,
    V: AsRef<std::ffi::OsStr>,
{
    unsafe {
        std::env::set_var(key, value);
    }
}

fn remove_env_var<K>(key: K)
where
    K: AsRef<std::ffi::OsStr>,
{
    unsafe {
        std::env::remove_var(key);
    }
}

fn launch_status(
    status: &str,
    message: &str,
    debug_port: u16,
    helper_port: u16,
    app_dir: &Path,
) -> LaunchStatus {
    LaunchStatus {
        status: status.to_string(),
        message: message.to_string(),
        started_at_ms: now_ms(),
        debug_port: Some(debug_port),
        helper_port: Some(helper_port),
        codex_app: Some(app_dir.to_string_lossy().to_string()),
    }
}

fn injection_failure_message(error: &anyhow::Error) -> String {
    let detail = error.to_string();
    if cfg!(target_os = "windows") {
        format!(
            "Codex launched, but the CodexAssistant enhancement bridge could not attach to the DevTools Protocol port. This often means a VPN or firewall WFP rule is blocking Windows TCP loopback. First try non-destructive checks: pause or quit the VPN client, allow localhost/127.0.0.1 traffic in the VPN or firewall settings, or enable split tunneling/local-network access. CodexAssistant does not change system network settings automatically. Diagnostic: {detail}"
        )
    } else {
        format!(
            "Codex launched, but the CodexAssistant enhancement bridge could not attach. Diagnostic: {detail}"
        )
    }
}

/// Verify TCP loopback works on this machine before we waste time launching Codex
/// and waiting 60s for `--remote-debugging-port` to be reachable. On Windows, VPN
/// drivers (WireGuard/Wintun, Cisco AnyConnect, Zscaler) commonly install WFP
/// kill-switch filters that silently drop 127.0.0.1 SYN packets — `listen()` and
/// `bind()` still succeed, so the symptom looks like Codex is broken when it isn't.
///
/// Some Chinese-market HIPS suites (notably Tencent QQ PC Manager:
/// `QQPCRTP` / `qmbsrv`) also drop 127.0.0.1 SYN for unsigned binaries that
/// lack a per-program ALLOW rule. When the first probe round fails on Windows
/// we attempt a one-time self-heal: register an explicit Windows Firewall
/// allow rule for the launcher binary (UAC-elevated), then retry. This is a
/// compliant fix — we only widen the allow-list for our own exe, never
/// disable the user's security software.
pub async fn preflight_loopback_reachable() -> anyhow::Result<()> {
    let initial = run_loopback_probe_rounds().await;
    if initial.is_ok() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        match try_loopback_self_heal_windows().await {
            Ok(true) => {
                let retry = run_loopback_probe_rounds().await;
                if retry.is_ok() {
                    let _ = crate::diagnostic_log::append_diagnostic_log(
                        "loopback.self_heal.success",
                        serde_json::json!({}),
                    );
                    return Ok(());
                }
                let _ = crate::diagnostic_log::append_diagnostic_log(
                    "loopback.self_heal.no_effect",
                    serde_json::json!({
                        "diagnostic": retry.as_ref().err().map(|e| e.to_string()).unwrap_or_default(),
                    }),
                );
                return retry;
            }
            Ok(false) => {
                // self-heal not applicable (e.g. rules already present, or
                // we already retried this binary this session) — fall through.
            }
            Err(e) => {
                let _ = crate::diagnostic_log::append_diagnostic_log(
                    "loopback.self_heal.failed",
                    serde_json::json!({ "error": e.to_string() }),
                );
            }
        }
    }

    initial
}

#[cfg(target_os = "windows")]
async fn try_loopback_self_heal_windows() -> anyhow::Result<bool> {
    use std::sync::OnceLock;
    static ALREADY_TRIED: OnceLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> =
        OnceLock::new();

    let exe = std::env::current_exe()
        .context("self_heal: std::env::current_exe failed")?;
    let canonical = exe.canonicalize().unwrap_or(exe);

    let tried = ALREADY_TRIED.get_or_init(Default::default);
    {
        let mut guard = tried.lock().expect("loopback self-heal mutex poisoned");
        if guard.contains(&canonical) {
            return Ok(false);
        }
        guard.insert(canonical.clone());
    }

    if crate::windows_integration::loopback_firewall_rules_present(&canonical) {
        // Rules exist; QQPC may be filtering at a layer the rule doesn't cover.
        // Skip re-prompting for UAC.
        return Ok(false);
    }

    let exe_for_blocking = canonical.clone();
    let blocking = tokio::task::spawn_blocking(move || {
        crate::windows_integration::ensure_loopback_firewall_allow(&exe_for_blocking)
    })
    .await
    .map_err(|e| anyhow::anyhow!("self_heal: spawn_blocking join failed: {e}"))?;
    blocking?;
    Ok(true)
}

async fn run_loopback_probe_rounds() -> anyhow::Result<()> {
    use std::net::Ipv4Addr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 1..=PREFLIGHT_LOOPBACK_ATTEMPTS {
        let listener = match tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await {
            Ok(listener) => listener,
            Err(error) => {
                last_error = Some(anyhow::anyhow!(
                    "loopback pre-flight attempt {attempt}: failed to bind 127.0.0.1:0: {error}"
                ));
                tokio::time::sleep(PREFLIGHT_LOOPBACK_RETRY_INTERVAL).await;
                continue;
            }
        };
        let port = listener.local_addr()?.port();

        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let _ = stream.write_all(b"ok").await;
                let _ = stream.shutdown().await;
            }
        });

        let probe = async {
            let mut stream = tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await?;
            let mut buf = [0u8; 2];
            stream.read_exact(&mut buf).await?;
            anyhow::Ok(())
        };

        let outcome = tokio::time::timeout(PREFLIGHT_LOOPBACK_TIMEOUT, probe).await;
        server.abort();

        match outcome {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(error)) => {
                last_error = Some(anyhow::anyhow!(
                    "loopback pre-flight attempt {attempt}: TCP connect to 127.0.0.1 failed: {error}"
                ));
            }
            Err(_) => {
                last_error = Some(anyhow::anyhow!(
                    "loopback pre-flight attempt {attempt}: TCP connect to 127.0.0.1 timed out after {}ms",
                    PREFLIGHT_LOOPBACK_TIMEOUT.as_millis()
                ));
            }
        }

        if attempt < PREFLIGHT_LOOPBACK_ATTEMPTS {
            tokio::time::sleep(PREFLIGHT_LOOPBACK_RETRY_INTERVAL).await;
        }
    }

    Err(anyhow::anyhow!(loopback_preflight_message(
        &last_error
            .unwrap_or_else(|| anyhow::anyhow!("loopback pre-flight failed without a diagnostic"))
            .to_string()
    )))
}

const PREFLIGHT_LOOPBACK_ATTEMPTS: u32 = 3;
const PREFLIGHT_LOOPBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2500);
const PREFLIGHT_LOOPBACK_RETRY_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(250);

fn loopback_preflight_message(detail: &str) -> String {
    if cfg!(target_os = "windows") {
        format!(
            "Windows TCP loopback (127.0.0.1) is unreachable on this machine, so the CodexAssistant launcher cannot talk to Codex's DevTools Protocol port. This is commonly caused by VPN or firewall WFP rules dropping localhost traffic. Try non-destructive recovery first: pause or quit the VPN client, allow localhost/127.0.0.1 in the VPN or firewall settings, or enable split tunneling/local-network access. CodexAssistant does not change system network settings automatically. Diagnostic: {detail}"
        )
    } else {
        format!("TCP loopback pre-flight failed: {detail}")
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_loopback_message_prefers_non_destructive_network_guidance() {
        let message = loopback_preflight_message("timeout");

        assert!(message.contains("non-destructive"));
        assert!(message.contains("does not change system network settings automatically"));
        assert!(!message.contains("Disable-NetAdapter"));
        assert!(!message.contains("netsh"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_injection_failure_message_avoids_adapter_disable_guidance() {
        let message = injection_failure_message(&anyhow::anyhow!("timeout"));

        assert!(message.contains("localhost/127.0.0.1"));
        assert!(message.contains("does not change system network settings automatically"));
        assert!(!message.contains("Disable-NetAdapter"));
        assert!(!message.contains("netsh"));
    }
}

async fn record_early_failure(
    status_store: &StatusStore,
    hooks: &Arc<dyn LaunchHooks>,
    debug_port: u16,
    helper_port: u16,
    app_dir: &Path,
    error: anyhow::Error,
    stage: &str,
) -> anyhow::Error {
    let message = error.to_string();
    let failure = launch_status("failed", &message, debug_port, helper_port, app_dir);
    let _ = status_store.save_latest(&failure);
    let _ = crate::diagnostic_log::append_diagnostic_log(
        "launcher.early_failure",
        serde_json::json!({
            "stage": stage,
            "message": message,
            "debug_port": debug_port,
            "helper_port": helper_port,
            "app_dir": app_dir.to_string_lossy().to_string(),
        }),
    );
    hooks.write_status("failed").await;
    error
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn command_line_arguments(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote_windows_argument(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_windows_argument(arg: &str) -> String {
    if !arg.is_empty() && !arg.bytes().any(|byte| matches!(byte, b' ' | b'\t' | b'"')) {
        return arg.to_string();
    }
    let mut output = String::from("\"");
    let mut backslashes = 0;
    for ch in arg.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                output.push_str(&"\\".repeat(backslashes * 2 + 1));
                output.push('"');
                backslashes = 0;
            }
            _ => {
                output.push_str(&"\\".repeat(backslashes));
                output.push(ch);
                backslashes = 0;
            }
        }
    }
    output.push_str(&"\\".repeat(backslashes * 2));
    output.push('"');
    output
}

#[cfg(not(windows))]
pub async fn activate_packaged_app(
    _app_user_model_id: &str,
    _arguments: &str,
) -> anyhow::Result<u32> {
    anyhow::bail!("Packaged app activation is only supported on Windows")
}

#[cfg(windows)]
pub async fn activate_packaged_app(
    app_user_model_id: &str,
    arguments: &str,
) -> anyhow::Result<u32> {
    let app_user_model_id = app_user_model_id.to_string();
    let arguments = arguments.to_string();
    tokio::task::spawn_blocking(move || {
        activate_packaged_app_blocking(&app_user_model_id, &arguments)
    })
    .await
    .context("packaged app activation task failed")?
}

#[cfg(windows)]
fn activate_packaged_app_blocking(app_user_model_id: &str, arguments: &str) -> anyhow::Result<u32> {
    use windows::Win32::System::Com::{
        CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    };
    use windows::Win32::UI::Shell::{ApplicationActivationManager, IApplicationActivationManager};
    use windows::core::HSTRING;

    unsafe {
        let coinit = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let should_uninitialize = coinit.is_ok();
        coinit.ok().or_else(|error| {
            const RPC_E_CHANGED_MODE: i32 = -2147417850;
            if error.code().0 == RPC_E_CHANGED_MODE {
                Ok(())
            } else {
                Err(error)
            }
        })?;

        let result: windows::core::Result<u32> = (|| {
            let manager: IApplicationActivationManager =
                CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)?;
            let process_id = manager.ActivateApplication(
                &HSTRING::from(app_user_model_id),
                &HSTRING::from(arguments),
                windows::Win32::UI::Shell::ACTIVATEOPTIONS(0),
            )?;
            Ok(process_id)
        })();

        if should_uninitialize {
            CoUninitialize();
        }
        result.map_err(Into::into)
    }
}

pub mod test_support {
    use super::*;
    use tokio::sync::oneshot;

    pub struct HelperHandle {
        pub port: u16,
        pub shutdown: oneshot::Sender<()>,
    }

    pub async fn spawn_helper_listener() -> HelperHandle {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let (tx, mut rx) = oneshot::channel();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => break,
                    accepted = listener.accept() => {
                        if let Ok((stream, addr)) = accepted {
                            tokio::spawn(async move {
                                let _ = handle_helper_connection(stream, Some(addr)).await;
                            });
                        }
                    }
                }
            }
        });
        HelperHandle { port, shutdown: tx }
    }

    pub async fn shutdown_helper_listener(shutdown: oneshot::Sender<()>) {
        let _ = shutdown.send(());
    }
}
