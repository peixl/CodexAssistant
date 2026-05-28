# scripts/dev

Local developer helpers. **Not used by CI or release packaging.** These exist
because the project is Windows-first and a few corners of the local toolchain
(MSVC, behind-the-Firewall cargo network) need a bit of extra hand-holding
that the standard `cargo build` flow doesn't provide.

## `build-msvc.ps1` / `build-msvc.cmd`

Wraps `vcvars64.bat` so `cargo build` can find `link.exe`, then runs the
project. Resolves the Visual Studio install via `vswhere.exe` so the script
is portable across VS editions / install years.

```pwsh
pwsh -File scripts\dev\build-msvc.ps1            # debug build
pwsh -File scripts\dev\build-msvc.ps1 -Release   # release build
pwsh -File scripts\dev\build-msvc.ps1 -CargoArgs "test --workspace"
```

The `.cmd` wrapper is for users whose default shell is `cmd.exe`; it just
calls the `.ps1` with the same args.

## `prefetch-crates.ps1`

Recovery tool for one specific situation: cargo's libcurl on Windows can fail
DNS resolution for the configured registry mirror (e.g. `tuna`) when host
security software (notably Tencent PC Manager / QQPCRTP) intercepts the
launcher's network calls. When that happens you get a `getaddrinfo() thread
failed to start` cascade.

This script reads a list of `<name>-<version>.crate` filenames (one per line)
and downloads each from `static.crates.io` directly via PowerShell's
`Invoke-WebRequest` (which uses .NET's HTTP stack, untouched by the issue),
dropping the files into the cargo registry cache directory. After running
it, `cargo build --offline` succeeds against the local cache.

```pwsh
pwsh -File scripts\dev\prefetch-crates.ps1 .\missing.txt
```

Build `missing.txt` from `Cargo.lock` minus what's already in
`~\.cargo\registry\cache\<mirror-id>\`. Most contributors will never need
this — only break-glass for blocked networks.
