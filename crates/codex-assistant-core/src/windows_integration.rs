#[cfg(windows)]
use std::ffi::{OsStr, OsString};
#[cfg(windows)]
use std::iter::once;
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::path::PathBuf;

#[cfg(windows)]
use anyhow::Context;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IPersistFile,
};
#[cfg(windows)]
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
#[cfg(windows)]
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegCreateKeyW, RegDeleteKeyW,
    RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
    TerminateProcess,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, IShellLinkW, KF_FLAG_DEFAULT, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    SHGetKnownFolderPath, ShellExecuteExW, ShellExecuteW, ShellLink,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWMINNOACTIVE};
#[cfg(windows)]
use windows::core::{Interface, PCWSTR, PWSTR};

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProcessInfo {
    pub process_id: u32,
    pub parent_process_id: u32,
    pub exe_file: String,
    pub executable_path: Option<PathBuf>,
}

#[cfg(windows)]
pub struct ComApartment;

#[cfg(windows)]
impl ComApartment {
    pub fn init() -> windows::core::Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}

#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutSpec {
    pub path: PathBuf,
    pub target: PathBuf,
    pub arguments: String,
    pub working_directory: Option<PathBuf>,
    pub description: String,
    pub icon: Option<PathBuf>,
    pub show_minimized: bool,
}

#[cfg(windows)]
pub fn create_shortcut(spec: &ShortcutSpec) -> anyhow::Result<()> {
    if let Some(parent) = spec.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _com = ComApartment::init().context("初始化 COM 失败")?;
    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .context("创建 ShellLink COM 对象失败")?;
        shell_link
            .SetPath(PCWSTR(wide_null(spec.target.as_os_str()).as_ptr()))
            .context("设置快捷方式目标失败")?;
        shell_link
            .SetArguments(PCWSTR(wide_null(spec.arguments.as_str()).as_ptr()))
            .context("设置快捷方式参数失败")?;
        if let Some(working_directory) = &spec.working_directory {
            shell_link
                .SetWorkingDirectory(PCWSTR(wide_null(working_directory.as_os_str()).as_ptr()))
                .context("设置快捷方式工作目录失败")?;
        }
        shell_link
            .SetDescription(PCWSTR(wide_null(spec.description.as_str()).as_ptr()))
            .context("设置快捷方式描述失败")?;
        if let Some(icon) = &spec.icon {
            shell_link
                .SetIconLocation(PCWSTR(wide_null(icon.as_os_str()).as_ptr()), 0)
                .context("设置快捷方式图标失败")?;
        }
        if spec.show_minimized {
            shell_link
                .SetShowCmd(SW_SHOWMINNOACTIVE)
                .context("设置快捷方式窗口模式失败")?;
        }
        let persist_file: IPersistFile = shell_link.cast().context("获取 IPersistFile 失败")?;
        persist_file
            .Save(PCWSTR(wide_null(spec.path.as_os_str()).as_ptr()), true)
            .context("保存快捷方式失败")?;
    }
    Ok(())
}

#[cfg(windows)]
pub fn desktop_dir() -> Option<PathBuf> {
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None).ok()?;
        let value = path.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(path.as_ptr().cast()));
        value
    }
}

#[cfg(windows)]
pub fn open_url(url: &str) -> anyhow::Result<()> {
    let operation = wide_null("open");
    let file = wide_null(url);
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWMINNOACTIVE,
        )
    };
    let code = result.0 as isize;
    if code <= 32 {
        anyhow::bail!("ShellExecuteW returned {code}");
    }
    Ok(())
}

/// Run `cmd.exe /C <cmd>` elevated via UAC and wait for it. The shell window
/// is hidden so the user only sees the consent prompt. Returns the child's
/// exit code, or an error if the user declined elevation or the shell could
/// not be started.
#[cfg(windows)]
pub fn run_elevated_cmd(cmd_line: &str) -> anyhow::Result<u32> {
    use windows::Win32::Foundation::WAIT_OBJECT_0;
    use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};

    let verb = wide_null("runas");
    let file = wide_null("cmd.exe");
    let params = wide_null(format!("/C {cmd_line}"));

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };

    unsafe { ShellExecuteExW(&mut info) }
        .map_err(|e| anyhow::anyhow!("ShellExecuteExW(runas cmd.exe) failed: {e}"))?;

    if info.hProcess.is_invalid() {
        anyhow::bail!("ShellExecuteExW returned no process handle (user likely declined UAC)");
    }
    let _guard = HandleGuard(info.hProcess);

    let wait = unsafe { WaitForSingleObject(info.hProcess, INFINITE) };
    if wait != WAIT_OBJECT_0 {
        anyhow::bail!("WaitForSingleObject returned {wait:?}");
    }

    let mut exit_code: u32 = 0;
    unsafe { GetExitCodeProcess(info.hProcess, &mut exit_code) }
        .map_err(|e| anyhow::anyhow!("GetExitCodeProcess failed: {e}"))?;
    Ok(exit_code)
}

/// Strip the Windows verbatim path prefix (`\\?\` or `\\?\UNC\`) so the path is
/// safe to hand to legacy Win32 tools that don't accept extended-length paths.
/// `std::env::current_exe().canonicalize()` returns `\\?\C:\...` on Windows,
/// and `netsh advfirewall firewall add rule program=...` refuses such inputs
/// with exit code 1, which is the symptom that drove this helper.
#[cfg(windows)]
pub fn simplify_windows_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        // Only strip when the remainder looks like a drive-letter path; leave
        // device-namespace paths (e.g. `\\?\Volume{...}`) untouched.
        if rest.len() >= 2 && rest.as_bytes()[0].is_ascii_alphabetic() && rest.as_bytes()[1] == b':'
        {
            return rest.to_string();
        }
    }
    path.to_string()
}

/// Add Inbound + Outbound `Allow` Windows Firewall rules scoped to `exe_path`,
/// requesting UAC elevation. Existing rules with the same names are removed
/// first so the call is idempotent. Returns `Ok(())` on success.
///
/// This is the recovery hook used when [`crate::launcher::preflight_loopback_reachable`]
/// fails on Windows: a third-party HIPS/AV (Tencent QQ PC Manager observed in
/// the field, also some endpoint-protection suites) installs WFP filters that
/// silently drop 127.0.0.1 SYN packets for unsigned binaries that lack a
/// per-program firewall allow rule. Adding our own ALLOW rule for the
/// launcher / helper exe takes precedence over the generic block and lets
/// loopback flow normally.
#[cfg(windows)]
pub fn ensure_loopback_firewall_allow(exe_path: &std::path::Path) -> anyhow::Result<()> {
    let exe = exe_path
        .canonicalize()
        .unwrap_or_else(|_| exe_path.to_path_buf());
    let raw = exe.to_string_lossy().to_string();
    let exe_str = simplify_windows_path(&raw);
    if exe_str.contains('"') {
        anyhow::bail!("refusing to build firewall command for path containing quote: {exe_str}");
    }
    let stem = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("CodexAssistant");
    let in_name = format!("CodexAssistant Loopback {stem} In");
    let out_name = format!("CodexAssistant Loopback {stem} Out");

    // Capture netsh stdout/stderr to a temp log so callers can see *why*
    // netsh refused the rule (path rejected, profile invalid, etc.) instead
    // of just an opaque exit code.
    let log_path = std::env::temp_dir().join("codex-assistant-loopback-self-heal.log");
    let log_str = log_path.to_string_lossy().to_string();
    if log_str.contains('"') {
        anyhow::bail!("refusing to build firewall command for log path containing quote");
    }
    // Best-effort: clear any prior log so we only surface the current run's output.
    let _ = std::fs::remove_file(&log_path);

    let mut script = String::from("(");
    script.push_str(&format!(
        "netsh advfirewall firewall delete rule name=\"{in_name}\" 2>&1 & "
    ));
    script.push_str(&format!(
        "netsh advfirewall firewall delete rule name=\"{out_name}\" 2>&1 & "
    ));
    script.push_str(&format!(
        "netsh advfirewall firewall add rule name=\"{in_name}\" dir=in action=allow program=\"{exe_str}\" enable=yes profile=any 2>&1 && "
    ));
    script.push_str(&format!(
        "netsh advfirewall firewall add rule name=\"{out_name}\" dir=out action=allow program=\"{exe_str}\" enable=yes profile=any 2>&1"
    ));
    script.push_str(&format!(") > \"{log_str}\" 2>&1"));

    let exit = run_elevated_cmd(&script)?;
    if exit != 0 {
        let detail = std::fs::read_to_string(&log_path).unwrap_or_default();
        let detail = detail.trim();
        if detail.is_empty() {
            anyhow::bail!("netsh advfirewall add rule exited with code {exit}");
        } else {
            anyhow::bail!(
                "netsh advfirewall add rule exited with code {exit}; netsh said: {detail}"
            );
        }
    }
    let _ = std::fs::remove_file(&log_path);
    Ok(())
}

#[cfg(windows)]
pub fn loopback_firewall_rules_present(exe_path: &std::path::Path) -> bool {
    let exe = exe_path
        .canonicalize()
        .unwrap_or_else(|_| exe_path.to_path_buf());
    let stem = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("CodexAssistant");
    let in_name = format!("CodexAssistant Loopback {stem} In");
    let out_name = format!("CodexAssistant Loopback {stem} Out");
    let check = |name: &str| -> bool {
        std::process::Command::new("netsh")
            .args([
                "advfirewall",
                "firewall",
                "show",
                "rule",
                &format!("name={name}"),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| {
                o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("No rules match")
            })
            .unwrap_or(false)
    };
    check(&in_name) && check(&out_name)
}

#[cfg(all(test, windows))]
mod tests {
    use super::simplify_windows_path;

    #[test]
    fn simplify_strips_drive_letter_verbatim_prefix() {
        assert_eq!(
            simplify_windows_path(r"\\?\C:\Program Files\CodexAssistant\app.exe"),
            r"C:\Program Files\CodexAssistant\app.exe"
        );
    }

    #[test]
    fn simplify_rewrites_unc_verbatim_prefix() {
        assert_eq!(
            simplify_windows_path(r"\\?\UNC\server\share\path\app.exe"),
            r"\\server\share\path\app.exe"
        );
    }

    #[test]
    fn simplify_leaves_non_verbatim_paths_alone() {
        assert_eq!(
            simplify_windows_path(r"C:\Program Files\CodexAssistant\app.exe"),
            r"C:\Program Files\CodexAssistant\app.exe"
        );
        assert_eq!(
            simplify_windows_path(r"\\server\share\path\app.exe"),
            r"\\server\share\path\app.exe"
        );
    }

    #[test]
    fn simplify_leaves_device_namespace_paths_alone() {
        // `\\?\Volume{...}` and similar are not drive-letter paths; leave
        // them untouched rather than producing an invalid simplification.
        assert_eq!(
            simplify_windows_path(r"\\?\Volume{12345678-1234-1234-1234-123456789abc}\app.exe"),
            r"\\?\Volume{12345678-1234-1234-1234-123456789abc}\app.exe"
        );
    }
}

#[cfg(windows)]
pub fn set_current_user_string_value(subkey: &str, name: &str, value: &str) -> anyhow::Result<()> {
    with_created_current_user_key(subkey, |key| {
        let value = wide_null(value);
        let bytes = slice_as_u8(&value);
        unsafe {
            RegSetValueExW(
                key,
                PCWSTR(wide_null(name).as_ptr()),
                None,
                REG_SZ,
                Some(bytes),
            )
        }
        .ok()
        .with_context(|| format!("写入注册表值 {subkey}\\{name} 失败"))
    })
}

#[cfg(windows)]
pub fn delete_current_user_value(subkey: &str, name: &str) -> anyhow::Result<()> {
    let subkey = wide_null(subkey);
    let name = wide_null(name);
    let mut key = HKEY::default();
    if unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            None,
            KEY_SET_VALUE,
            &mut key,
        )
    }
    .is_err()
    {
        return Ok(());
    }
    let _guard = RegistryKeyGuard(key);
    unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) }
        .ok()
        .or_else(|_| Ok(()))
}

#[cfg(windows)]
pub fn delete_current_user_key(subkey: &str) -> anyhow::Result<()> {
    let subkey = wide_null(subkey);
    unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr())) }
        .ok()
        .or_else(|_| Ok(()))
}

#[cfg(windows)]
pub fn enumerate_processes() -> Vec<WindowsProcessInfo> {
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    if snapshot.is_invalid() {
        return Vec::new();
    }
    let _guard = HandleGuard(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut processes = Vec::new();
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_err() {
        return Vec::new();
    }
    loop {
        let process_id = entry.th32ProcessID;
        processes.push(WindowsProcessInfo {
            process_id,
            parent_process_id: entry.th32ParentProcessID,
            exe_file: nul_terminated_wide_to_string(&entry.szExeFile),
            executable_path: query_process_image_path(process_id),
        });
        if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
            break;
        }
    }
    processes
}

#[cfg(windows)]
pub fn terminate_process(process_id: u32) -> bool {
    let Ok(handle) = (unsafe {
        OpenProcess(
            PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            process_id,
        )
    }) else {
        return false;
    };
    if handle.is_invalid() {
        return false;
    }
    let _guard = HandleGuard(handle);
    unsafe { TerminateProcess(handle, 0) }.is_ok()
}

#[cfg(windows)]
fn query_process_image_path(process_id: u32) -> Option<PathBuf> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()? };
    if handle.is_invalid() {
        return None;
    }
    let _guard = HandleGuard(handle);
    let mut buffer = vec![0u16; MAX_PATH as usize * 4];
    let mut len = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            Default::default(),
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
        .ok()?;
    }
    Some(PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
}

#[cfg(windows)]
fn with_created_current_user_key<T>(
    subkey: &str,
    f: impl FnOnce(HKEY) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyW(
            HKEY_CURRENT_USER,
            PCWSTR(wide_null(subkey).as_ptr()),
            &mut key,
        )
    }
    .ok()
    .with_context(|| format!("打开注册表键 HKCU\\{subkey} 失败"))?;
    let _guard = RegistryKeyGuard(key);
    f(key)
}

#[cfg(windows)]
fn slice_as_u8(value: &[u16]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast::<u8>(), std::mem::size_of_val(value)) }
}

#[cfg(windows)]
fn wide_null(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(once(0)).collect()
}

#[cfg(windows)]
fn nul_terminated_wide_to_string(value: &[u16]) -> String {
    let len = value.iter().position(|ch| *ch == 0).unwrap_or(value.len());
    OsString::from_wide(&value[..len])
        .to_string_lossy()
        .to_string()
}

#[cfg(windows)]
struct HandleGuard(HANDLE);

#[cfg(windows)]
impl Drop for HandleGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[cfg(windows)]
struct RegistryKeyGuard(HKEY);

#[cfg(windows)]
impl Drop for RegistryKeyGuard {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}
