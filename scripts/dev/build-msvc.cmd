@echo off
REM Build CodexAssistant with the system MSVC toolchain.
REM Wrapper around build-msvc.ps1; pass any cargo args after `--`.
REM
REM Examples:
REM   scripts\dev\build-msvc.cmd
REM   scripts\dev\build-msvc.cmd -- test --workspace
REM   scripts\dev\build-msvc.cmd -Release

setlocal
set "SCRIPT_DIR=%~dp0"

REM Strip the leading `--` separator some users add.
if "%~1"=="--" shift

powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%build-msvc.ps1" %*
exit /b %ERRORLEVEL%
