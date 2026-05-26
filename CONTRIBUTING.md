# Contributing to CodexAssistant

Thank you for your interest in contributing to CodexAssistant!

## Development Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/peixl/CodexAssistant.git
   cd CodexAssistant
   ```

2. **Install toolchains**

   - Rust 1.85+ (the workspace uses `edition = "2024"`):
     ```bash
     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
     rustc --version  # should be >= 1.85
     ```
   - Node.js 20+ and npm — required by the Tauri manager frontend.
   - On Linux, install Tauri's system dependencies (`libwebkit2gtk-4.1-dev`,
     `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`,
     `libsoup-3.0-dev`). macOS and Windows have everything in the OS SDK.

3. **Install frontend dependencies**
   ```bash
   npm --prefix apps/codex-plus-manager ci
   ```

4. **Build the project**
   ```bash
   # Build the frontend once so `tauri::generate_context!` can resolve
   # apps/codex-plus-manager/dist/ at compile time.
   npm --prefix apps/codex-plus-manager run vite:build

   # Build all Rust crates and binaries in release mode.
   cargo build --release
   ```

5. **Run the manager in dev mode**
   ```bash
   npm --prefix apps/codex-plus-manager run dev
   ```

## Project Structure

```
CodexAssistant/
├── apps/
│   ├── codex-plus-launcher/    # Silent launcher binary (codex-assistant)
│   └── codex-plus-manager/     # Tauri manager (React + Rust)
│       ├── src/                # React + TypeScript UI
│       └── src-tauri/          # Tauri shell + commands
├── crates/
│   ├── codex-plus-core/        # Core launcher, CDP injection, settings, relay
│   └── codex-plus-data/        # SQLite adapter, provider sync, markdown export
├── assets/inject/              # JS injected into the Codex renderer via CDP
├── scripts/installer/          # macOS DMG + Windows NSIS installers
└── .github/workflows/          # CI + release pipelines
```

## Making Changes

1. **Create a feature branch**
   ```bash
   git checkout -b feat/your-feature-name
   ```

2. **Enable the repository git hooks (one-time per clone)**
   ```bash
   bash scripts/git-hooks/install.sh
   ```

   This sets `core.hooksPath` to `scripts/git-hooks`, which installs a
   `pre-push` hook that refuses force-pushes and deletions of `main`.
   Set `PROTECT_MAIN_BYPASS=1` if you ever genuinely need to override it.

3. **Make your changes**
   - Follow existing patterns; see [docs/superpowers/specs/](docs/superpowers/specs/) for design notes.
   - Add tests for new behaviour — Rust integration tests live under
     `crates/*/tests/` and `apps/codex-plus-manager/src-tauri/tests/`;
     frontend tests use vitest under `apps/codex-plus-manager/src/**/*.test.ts`.
   - Cross-platform code paths must be gated with
     `#[cfg(target_os = "windows" | "macos")]`.

4. **Run the full local check before pushing**
   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   npm --prefix apps/codex-plus-manager run check
   npm --prefix apps/codex-plus-manager run test
   ```

   These are the same gates CI enforces — see
   [.github/workflows/ci.yml](.github/workflows/ci.yml).

## Code Style

- Rust: `cargo fmt` + `clippy` with `-D warnings`. Prefer `anyhow::Result`
  at the binary/edge layer and concrete `thiserror` enums in the core crate.
- TypeScript: strict mode (`"strict": true` in `tsconfig.json`). Route all
  Tauri command calls through `src/lib/invoke.ts` so errors are normalized.
- Keep modules focused; one purpose per file. Avoid sprawling files —
  large files are a signal that boundaries need work.
- Comments only when the **why** is non-obvious. Don't restate what the code does.

## Pull Request Process

1. Fork the repository
2. Create your feature branch
3. Make your changes with adequate tests
4. Ensure all local gates pass (see above) and CI is green
5. Submit a pull request — the PR template asks for platform impact and
   a verification checklist; please fill both out

### CI tiers

CI is tuned to stay within free-tier GitHub Actions minutes. By default,
PRs and pushes only run Linux jobs. To opt a PR into the full
macOS + Windows matrix before merging, apply the `ci:full` label, or run
`gh workflow run ci.yml` against the branch. The full matrix also runs
automatically on tag pushes (`v*`).

### Cutting a release

1. Bump the workspace version in `Cargo.toml` and run `cargo update -w`.
2. Commit and push to `main`. Wait for Linux CI to go green.
3. Tag and push:
   ```bash
   git tag v$(awk -F'"' '/^version = / { print $2; exit }' Cargo.toml)
   git push origin --tags
   ```
   The tag push runs the full macOS + Windows matrix.
4. Create the GitHub Release (`gh release create vX.Y.Z --generate-notes`).
   `release-assets.yml` builds the macOS DMG + Windows NSIS installer
   and attaches them automatically.

## Reporting Issues

- Use GitHub Issues for bug reports and feature requests; the forms
  under `.github/ISSUE_TEMPLATE/` will guide you.
- Always include OS + arch (Windows x64 / macOS arm64 / macOS x64),
  CodexAssistant version (shown in 更多 → 关于), and the relevant lines
  from the diagnostics bundle.

## Security

Do **not** open public issues for security vulnerabilities. See
[SECURITY.md](SECURITY.md) for private reporting channels.

## License

By contributing, you agree that your contributions will be licensed
under the [MIT License](LICENSE).

