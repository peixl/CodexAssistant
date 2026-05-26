use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::user_scripts::UserScriptManager;

pub const DEFAULT_MARKET_INDEX_URL: &str =
    "https://raw.githubusercontent.com/peixl/CodexAssistantScriptMarket/main/index.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScriptMarketManifest {
    pub version: u64,
    pub updated_at: Option<String>,
    pub scripts: Vec<MarketScript>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketScript {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub homepage: String,
    pub script_url: String,
    #[serde(default)]
    pub sha256: String,
}

pub fn parse_market_manifest(raw: Value) -> anyhow::Result<ScriptMarketManifest> {
    let version = raw.get("version").and_then(Value::as_u64).unwrap_or(1);
    let updated_at = raw
        .get("updated_at")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let raw_scripts: Vec<Value> = raw
        .get("scripts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let raw_total = raw_scripts.len();
    let mut dropped_ids: Vec<String> = Vec::new();
    let scripts: Vec<MarketScript> = raw_scripts
        .into_iter()
        .filter_map(|entry| {
            let candidate_id = entry
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            let has_sha256 = entry
                .get("sha256")
                .and_then(Value::as_str)
                .map(str::trim)
                .is_some_and(|s| !s.is_empty());
            match parse_market_script(entry) {
                Some(parsed) => Some(parsed),
                None => {
                    if !has_sha256 && !candidate_id.is_empty() {
                        dropped_ids.push(candidate_id);
                    }
                    None
                }
            }
        })
        .collect();
    if !dropped_ids.is_empty() {
        let _ = crate::diagnostic_log::append_diagnostic_log(
            "security.script_market_dropped_no_sha256",
            serde_json::json!({
                "total": raw_total,
                "kept": scripts.len(),
                "dropped_ids": dropped_ids,
            }),
        );
    }
    Ok(ScriptMarketManifest {
        version,
        updated_at,
        scripts,
    })
}

pub async fn fetch_market_manifest(url: &str) -> anyhow::Result<ScriptMarketManifest> {
    let client =
        crate::http_client::proxied_client(&format!("CodexAssistant/{}", crate::version::VERSION))?;
    let raw = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to request script market index {url}"))?
        .error_for_status()
        .with_context(|| format!("script market index returned an error status {url}"))?
        .json::<Value>()
        .await
        .context("failed to decode script market index JSON")?;
    parse_market_manifest(raw)
}

pub async fn download_script(url: &str) -> anyhow::Result<Vec<u8>> {
    let client =
        crate::http_client::proxied_client(&format!("CodexAssistant/{}", crate::version::VERSION))?;
    Ok(client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to request script {url}"))?
        .error_for_status()
        .with_context(|| format!("script download returned an error status {url}"))?
        .bytes()
        .await
        .context("failed to read script download body")?
        .to_vec())
}

pub fn install_market_script_content(
    manager: &UserScriptManager,
    script: &MarketScript,
    content: &[u8],
) -> anyhow::Result<()> {
    verify_sha256(script, content)?;
    let path = manager.user_script_path_for_market_id(&script.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create user script directory {}",
                parent.display()
            )
        })?;
    }
    crate::settings::atomic_write(&path, content)
        .with_context(|| format!("failed to write script {}", path.display()))?;
    manager.record_market_install(script)?;
    Ok(())
}

pub async fn install_market_script(
    manager: &UserScriptManager,
    script: &MarketScript,
) -> anyhow::Result<()> {
    let content = download_script(&script.script_url).await?;
    install_market_script_content(manager, script, &content)
}

fn parse_market_script(raw: Value) -> Option<MarketScript> {
    let id = required_string(&raw, "id")?;
    let name = required_string(&raw, "name")?;
    let version = required_string(&raw, "version")?;
    let script_url = required_string(&raw, "script_url")?;
    let sha256 = required_string(&raw, "sha256")?;
    Some(MarketScript {
        id,
        name,
        description: optional_string(&raw, "description"),
        version,
        author: optional_string(&raw, "author"),
        tags: raw
            .get("tags")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        homepage: optional_string(&raw, "homepage"),
        script_url,
        sha256,
    })
}

fn required_string(raw: &Value, key: &str) -> Option<String> {
    raw.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn optional_string(raw: &Value, key: &str) -> String {
    raw.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

fn verify_sha256(script: &MarketScript, content: &[u8]) -> anyhow::Result<()> {
    let expected = script.sha256.trim().to_ascii_lowercase();
    anyhow::ensure!(!expected.is_empty(), "script {} missing sha256", script.id);
    let actual = to_hex(&Sha256::digest(content));
    if actual != expected {
        let _ = crate::diagnostic_log::append_diagnostic_log(
            "security.script_market_sha256_mismatch",
            serde_json::json!({
                "id": script.id,
                "expected": expected,
                "actual": actual,
            }),
        );
        anyhow::bail!("script {} sha256 mismatch", script.id);
    }
    Ok(())
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod hardening_tests {
    use super::*;
    use serde_json::json;

    fn script_obj(id: &str, with_sha: bool) -> Value {
        let mut obj = json!({
            "id": id,
            "name": id,
            "version": "1.0.0",
            "script_url": format!("https://example.com/{id}.js"),
        });
        if with_sha {
            obj["sha256"] = json!("aa".repeat(32));
        }
        obj
    }

    #[test]
    fn manifest_drops_entries_without_sha256() {
        let raw = json!({
            "version": 1,
            "scripts": [script_obj("good", true), script_obj("bad", false)],
        });
        let manifest = parse_market_manifest(raw).expect("parse");
        let ids: Vec<&str> = manifest.scripts.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["good"]);
    }

    #[test]
    fn verify_sha256_rejects_empty() {
        let script = MarketScript {
            id: "x".into(),
            name: "x".into(),
            description: String::new(),
            version: "1".into(),
            author: String::new(),
            tags: vec![],
            homepage: String::new(),
            script_url: "https://example.com/x.js".into(),
            sha256: String::new(),
        };
        assert!(verify_sha256(&script, b"content").is_err());
    }

    #[test]
    fn verify_sha256_rejects_mismatch() {
        let script = MarketScript {
            id: "x".into(),
            name: "x".into(),
            description: String::new(),
            version: "1".into(),
            author: String::new(),
            tags: vec![],
            homepage: String::new(),
            script_url: "https://example.com/x.js".into(),
            sha256: "00".repeat(32),
        };
        assert!(verify_sha256(&script, b"content").is_err());
    }
}
