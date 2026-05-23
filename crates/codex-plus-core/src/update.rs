use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const DEFAULT_REPOSITORY: &str = "peixl/CodexAssistant";
pub const DEFAULT_LATEST_JSON_URL: &str =
    "https://github.com/peixl/CodexAssistant/releases/latest/download/latest.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub body: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    #[serde(default)]
    pub asset_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_summary: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_sha256: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateInstall {
    pub release: Release,
    pub installer_path: PathBuf,
    pub launched: bool,
}

pub fn parse_version_tag(value: &str) -> anyhow::Result<Vec<u64>> {
    let normalized = value.trim().trim_start_matches(['v', 'V']);
    let mut digits = String::new();
    for ch in normalized.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            digits.push(ch);
        } else {
            break;
        }
    }
    if digits.is_empty() {
        anyhow::bail!("Invalid version tag: {value}");
    }
    digits
        .split('.')
        .map(|part| part.parse::<u64>().map_err(Into::into))
        .collect()
}

pub fn is_newer_version(candidate: &str, current: &str) -> anyhow::Result<bool> {
    let mut left = parse_version_tag(candidate)?;
    let mut right = parse_version_tag(current)?;
    let len = left.len().max(right.len());
    left.resize(len, 0);
    right.resize(len, 0);
    Ok(left > right)
}

pub fn release_from_github_payload(payload: &Value) -> anyhow::Result<Release> {
    let version = payload
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("release payload missing tag_name"))?
        .to_string();
    let assets = payload
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|asset| {
            Some((
                asset.get("name")?.as_str()?.to_string(),
                asset.get("browser_download_url")?.as_str()?.to_string(),
                None::<String>,
            ))
        })
        .collect::<Vec<_>>();
    let selected = select_update_asset(&assets);
    Ok(Release {
        version,
        url: payload
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        body: payload
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset_name: selected.as_ref().map(|asset| asset.name.clone()),
        asset_url: selected
            .as_ref()
            .map(|asset| asset.browser_download_url.clone()),
        asset_sha256: selected.and_then(|asset| asset.sha256),
    })
}

pub fn release_from_latest_json_payload(payload: &Value) -> anyhow::Result<Release> {
    let version = payload
        .get("version")
        .or_else(|| payload.get("tag_name"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("latest.json missing version"))?
        .to_string();
    let assets = payload
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|asset| {
            let name = asset.get("name")?.as_str()?.to_string();
            let url = asset
                .get("url")
                .or_else(|| asset.get("browser_download_url"))?
                .as_str()?
                .to_string();
            let sha256 = asset
                .get("sha256")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_ascii_lowercase);
            Some((name, url, sha256))
        })
        .collect::<Vec<_>>();
    let selected = select_update_asset(&assets);
    Ok(Release {
        version,
        url: payload
            .get("url")
            .or_else(|| payload.get("html_url"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        body: payload
            .get("body")
            .or_else(|| payload.get("release_summary"))
            .or_else(|| payload.get("notes"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset_name: selected.as_ref().map(|asset| asset.name.clone()),
        asset_url: selected
            .as_ref()
            .map(|asset| asset.browser_download_url.clone()),
        asset_sha256: selected.and_then(|asset| asset.sha256),
    })
}

pub fn select_update_asset(assets: &[(String, String, Option<String>)]) -> Option<ReleaseAsset> {
    let named = assets
        .iter()
        .filter(|(name, url, _)| !name.trim().is_empty() && !url.trim().is_empty())
        .collect::<Vec<_>>();
    for (name, url, sha256) in &named {
        let lower = name.to_ascii_lowercase();
        if platform_asset_rank(&lower) == 0 {
            return Some(ReleaseAsset {
                name: (*name).clone(),
                browser_download_url: (*url).clone(),
                sha256: sha256.clone(),
            });
        }
    }
    None
}

pub async fn fetch_latest_release(latest_json_url: &str) -> anyhow::Result<Release> {
    let client =
        crate::http_client::proxied_client(&format!("CodexAssistant/{}", crate::version::VERSION))?;
    let payload = client
        .get(latest_json_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    release_from_latest_json_payload(&payload)
}

pub async fn check_for_update(current_version: &str) -> anyhow::Result<UpdateCheck> {
    let release = fetch_latest_release(DEFAULT_LATEST_JSON_URL).await?;
    let update_available = is_newer_version(&release.version, current_version)?;
    Ok(UpdateCheck {
        current_version: current_version.to_string(),
        latest_version: Some(release.version),
        release_summary: release.body,
        asset_name: release.asset_name,
        asset_url: release.asset_url,
        asset_sha256: release.asset_sha256,
        update_available,
    })
}

pub async fn perform_update(
    release: &Release,
    download_dir: &Path,
) -> anyhow::Result<UpdateInstall> {
    let url = release
        .asset_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("没有可下载的 Release asset"))?;
    let bytes =
        crate::http_client::download_client(&format!("CodexAssistant/{}", crate::version::VERSION))?
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
    let installer_path = download_asset_to(release, &bytes, download_dir)?;
    validate_downloaded_installer(release, &installer_path, &bytes)?;
    launch_installer(&installer_path)?;
    Ok(UpdateInstall {
        release: release.clone(),
        installer_path,
        launched: true,
    })
}

pub fn download_asset_to(
    release: &Release,
    bytes: &[u8],
    download_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let name = release
        .asset_name
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("没有可下载的 Release asset"))?;
    let safe = safe_asset_name(name)?;
    std::fs::create_dir_all(download_dir)?;
    let path = download_dir.join(safe);
    std::fs::write(&path, bytes)?;
    Ok(path)
}

pub fn safe_asset_name(name: &str) -> anyhow::Result<String> {
    if name.trim().is_empty() {
        anyhow::bail!("非法 Release asset 文件名: {name}");
    }
    let path = Path::new(name);
    if path.components().count() != 1 {
        anyhow::bail!("非法 Release asset 文件名: {name}");
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("非法 Release asset 文件名: {name}"))?;
    if file_name == "." || file_name == ".." {
        anyhow::bail!("非法 Release asset 文件名: {name}");
    }
    Ok(file_name.to_string())
}

fn platform_asset_rank(name: &str) -> u8 {
    if cfg!(windows) && is_windows_installer_asset(name) {
        return 0;
    }
    if cfg!(target_os = "macos") && is_macos_installer_asset(name) {
        return 0;
    }
    2
}

fn is_windows_installer_asset(name: &str) -> bool {
    name.contains("codex")
        && (name.contains("plus") || name.contains("assistant"))
        && (name.ends_with(".msi")
            || name.ends_with("-setup.exe")
            || name.ends_with("_setup.exe")
            || name.ends_with("setup.exe")
            || name.ends_with("installer.exe"))
}

fn is_macos_installer_asset(name: &str) -> bool {
    name.contains("codex")
        && (name.contains("plus") || name.contains("assistant"))
        && name.ends_with(".dmg")
}

pub fn launch_installer(path: &Path) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(path)
            .creation_flags(crate::windows_integration::CREATE_NO_WINDOW)
            .spawn()
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("启动安装包失败：{error}"))
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("打开 DMG 失败：{error}"))
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = path;
        anyhow::bail!("当前平台不支持启动安装包")
    }
}

pub fn validate_downloaded_installer(
    release: &Release,
    installer_path: &Path,
    bytes: &[u8],
) -> anyhow::Result<()> {
    let expected = match release.asset_sha256.as_deref() {
        Some(sha) if !sha.trim().is_empty() => sha,
        _ => {
            let _ = std::fs::remove_file(installer_path);
            let _ = crate::diagnostic_log::append_diagnostic_log(
                "security.update_missing_sha256",
                serde_json::json!({
                    "version": release.version,
                    "asset_name": release.asset_name,
                }),
            );
            anyhow::bail!("更新包缺少校验和，已拒绝安装");
        }
    };
    if let Err(error) = verify_asset_sha256(expected, bytes) {
        let _ = std::fs::remove_file(installer_path);
        let _ = crate::diagnostic_log::append_diagnostic_log(
            "security.update_sha256_mismatch",
            serde_json::json!({
                "version": release.version,
                "asset_name": release.asset_name,
                "error": error.to_string(),
            }),
        );
        return Err(error);
    }
    Ok(())
}

pub fn verify_asset_sha256(expected_hex: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let expected = expected_hex.trim().to_ascii_lowercase();
    anyhow::ensure!(
        expected.len() == 64 && expected.bytes().all(|b| b.is_ascii_hexdigit()),
        "更新包校验失败：非法 sha256 长度或字符"
    );
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut actual = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut actual, "{byte:02x}").expect("writing to String never fails");
    }
    anyhow::ensure!(actual == expected, "更新包校验失败：sha256 不匹配");
    Ok(())
}

#[cfg(test)]
mod sha256_tests {
    use super::*;
    #[cfg(any(windows, target_os = "macos"))]
    use serde_json::json;

    #[cfg(any(windows, target_os = "macos"))]
    fn platform_asset_name(version: &str) -> String {
        if cfg!(windows) {
            format!("codex-plus_{version}_x64-setup.exe")
        } else {
            format!("codex-assistant_{version}_aarch64.dmg")
        }
    }

    #[test]
    #[cfg(any(windows, target_os = "macos"))]
    fn latest_json_parses_sha256() {
        let asset_name = platform_asset_name("1.4.0");
        let sha256_val = "ab".repeat(32);
        let payload = json!({
            "version": "1.4.0",
            "assets": [{
                "name": asset_name,
                "url": "https://example.com/x",
                "sha256": sha256_val
            }]
        });
        let release = release_from_latest_json_payload(&payload).expect("parse");
        assert_eq!(release.asset_name.as_deref(), Some(asset_name.as_str()));
        assert_eq!(release.asset_sha256.as_deref(), Some(&*"ab".repeat(32)));
    }

    #[test]
    #[cfg(any(windows, target_os = "macos"))]
    fn latest_json_missing_sha256_yields_none() {
        let asset_name = platform_asset_name("1.4.0");
        let payload = json!({
            "version": "1.4.0",
            "assets": [{
                "name": asset_name,
                "url": "https://example.com/x"
            }]
        });
        let release = release_from_latest_json_payload(&payload).expect("parse");
        assert!(release.asset_sha256.is_none());
    }

    #[test]
    fn verify_matches_real_sha256() {
        let body = b"hello world";
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        verify_asset_sha256(expected, body).expect("matches");
    }

    #[test]
    fn verify_rejects_mismatch() {
        let body = b"hello world";
        let expected = "00".repeat(32);
        let err = verify_asset_sha256(&expected, body).unwrap_err();
        assert!(err.to_string().contains("校验失败"), "{err}");
    }

    #[test]
    fn verify_rejects_bad_length() {
        assert!(verify_asset_sha256("abc", b"x").is_err());
    }

    #[test]
    fn verify_rejects_non_hex() {
        let expected = format!("{}gg", "a".repeat(62));
        assert!(verify_asset_sha256(&expected, b"x").is_err());
    }

    use tempfile::TempDir;

    fn release_with_sha(sha: Option<&str>) -> Release {
        Release {
            version: "1.4.0".into(),
            url: "https://example.com".into(),
            body: "".into(),
            asset_name: Some("codex-plus_1.4.0_x64-setup.exe".into()),
            asset_url: Some("https://example.com/x.exe".into()),
            asset_sha256: sha.map(str::to_string),
        }
    }

    #[test]
    fn validate_download_rejects_missing_sha256() {
        let dir = TempDir::new().unwrap();
        let release = release_with_sha(None);
        let installer = dir.path().join("x.exe");
        std::fs::write(&installer, b"payload").unwrap();
        let err = validate_downloaded_installer(&release, &installer, b"payload").unwrap_err();
        assert!(err.to_string().contains("缺少校验和"), "{err}");
        assert!(
            !installer.exists(),
            "installer should be removed on failure"
        );
    }

    #[test]
    fn validate_download_rejects_mismatch() {
        let dir = TempDir::new().unwrap();
        let release = release_with_sha(Some(&"00".repeat(32)));
        let installer = dir.path().join("x.exe");
        std::fs::write(&installer, b"payload").unwrap();
        let err = validate_downloaded_installer(&release, &installer, b"payload").unwrap_err();
        assert!(err.to_string().contains("校验失败"), "{err}");
        assert!(!installer.exists());
    }

    #[test]
    fn validate_download_accepts_match() {
        let dir = TempDir::new().unwrap();
        // sha256("payload") = 239f59ed55e737c77147cf55ad0c1b030b6d7ee748a7426952f9b852d5a935e5
        let release = release_with_sha(Some(
            "239f59ed55e737c77147cf55ad0c1b030b6d7ee748a7426952f9b852d5a935e5",
        ));
        let installer = dir.path().join("x.exe");
        std::fs::write(&installer, b"payload").unwrap();
        validate_downloaded_installer(&release, &installer, b"payload").expect("ok");
        assert!(installer.exists());
    }
}
