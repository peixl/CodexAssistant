# Security Policy

## Supported Versions

Only the latest minor release of CodexAssistant receives security fixes.

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

**Do not** open a public GitHub issue for security vulnerabilities.

Instead, report privately via one of:

- GitHub **Private Vulnerability Reporting** on this repository
  (Security tab → Report a vulnerability)
- Email: `1727532@qq.com` with subject prefix `[security] CodexAssistant`

Please include:

- A description of the vulnerability and its impact
- Steps to reproduce (PoC welcome)
- Affected version(s) and platform (Windows / macOS x64 / macOS arm64)
- Whether you wish to be credited in the fix announcement

### Response timeline

| Stage                | Target |
| -------------------- | ------ |
| Acknowledgement      | within 72h |
| Initial assessment   | within 7 days |
| Coordinated fix/release | within 30 days for high/critical |

We will keep you informed of progress and notify you when a fix is released.

## Scope

In scope:

- Code in `crates/`, `apps/`, `assets/inject/`
- Build pipeline in `.github/workflows/`
- Distributed installers (Windows `.exe`, macOS `.dmg`)

Out of scope:

- Third-party relay providers or services linked from the README
- The upstream Codex application itself
- Vulnerabilities requiring physical access or pre-existing local code execution

## Hardening notes

- Tauri commands validate URL schemes before opening external URLs
- Helper bridge uses a per-session token; the helper port is bound to `127.0.0.1` only
- Relay tokens are stored on disk with appropriate file permissions and never logged
- Auto-update validates downloaded payloads before replacing on-disk binaries
