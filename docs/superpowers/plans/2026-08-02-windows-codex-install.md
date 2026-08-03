# OBS Telegram Send for Windows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a Windows x64 OBS plugin and per-user local service that a Codex-guided installer can install without administrator access.

**Architecture:** Keep the plugin protocol and onboarding identical across platforms. Make the Rust agent’s private file handling and bundled Bot API path platform-aware; Windows installs the agent and Bot API in `%LOCALAPPDATA%\\OBS-Telegram-Send`, registers a current-user Scheduled Task, and places the OBS DLL in the user plugin directory under `%APPDATA%`.

**Tech Stack:** C++17, Qt 6, OBS 32.1.1 x64 SDK/runtime, Rust, PowerShell, GitHub Actions Windows runner, official Telegram Bot API.

## Global Constraints

- Target Windows 10/11 x64 and OBS Studio 32.1.1 x64 for the first release.
- Preserve the 2 GiB local Bot API upload capability; do not substitute Telegram cloud API.
- Do not store bot token, `api_id`, or `api_hash` in the package, repo, logs, or task arguments.
- Install only under the current user’s `%APPDATA%` and `%LOCALAPPDATA%`; no UAC/admin requirement.
- The public unsigned Windows bundle must disclose SmartScreen risk and be installed only by the client’s Codex session after it verifies the GitHub release checksum.

---

### Task 1: Make the agent runtime portable

**Files:**
- Modify: `agent/Cargo.toml`
- Modify: `agent/src/install_bearer.rs`
- Modify: `agent/src/telegram.rs`
- Test: `agent/tests/install_bearer_test.rs`
- Test: `agent/tests/telegram_test.rs`

**Interfaces:**
- Produces `InstallBearerStore::in_app_support()` that uses protected, current-user storage on macOS and Windows.
- Produces `LocalBotApiServer::packaged_executable_path()` that resolves the sibling `telegram-bot-api.exe` on Windows.

- [ ] Add Windows-compatible keyring support and conditional permission functions.
- [ ] Replace Unix-only imports with `cfg(unix)` guards and make Windows permission enforcement a no-op after current-user directory creation.
- [ ] Resolve the packaged Bot API path from the agent executable directory on Windows and retain the fixed macOS path.
- [ ] Add tests asserting platform executable naming and bearer creation/reload behavior.
- [ ] Run `cargo test --manifest-path agent/Cargo.toml` and `cargo clippy --manifest-path agent/Cargo.toml --all-targets -- -D warnings`.

### Task 2: Make the OBS plugin build on Windows x64

**Files:**
- Modify: `CMakeLists.txt`
- Modify: `plugin/CMakeLists.txt`
- Test: `plugin/tests/CMakeLists.txt`

**Interfaces:**
- Consumes `OBS_WINDOWS_ROOT`, `OBS_INCLUDE_DIR`, and `OBS_DEPENDENCIES_INCLUDE_DIR` from CI.
- Produces `obs-telegram-send.dll`, linking `obs.lib` and `obs-frontend-api.lib` on Windows.

- [ ] Add explicit macOS/Windows CMake branches instead of assuming an `.app` bundle.
- [ ] Set the Windows module target to a DLL without macOS bundle properties.
- [ ] Preserve existing tests and add the Windows target to the same test build.
- [ ] Configure and compile on `windows-2022` with the OBS 32.1.1 portable x64 archive and matching source headers.

### Task 3: Add a zero-admin Windows installer for Codex

**Files:**
- Create: `installer/windows/install-obs-telegram-send.ps1`
- Create: `installer/windows/uninstall-obs-telegram-send.ps1`
- Create: `installer/windows/package-layout-test.ps1`
- Create: `installer/windows/README.md`

**Interfaces:**
- Input bundle layout: `plugin/obs-telegram-send.dll`, `bin/obs-telegram-agent.exe`, `bin/telegram-bot-api.exe`, `checksums.sha256`.
- Installs plugin to `$env:APPDATA\\obs-studio\\plugins\\obs-telegram-send\\bin\\64bit` and runtime to `$env:LOCALAPPDATA\\OBS-Telegram-Send`.
- Registers task `OBS-Telegram-Send-Agent` for the current interactive user.

- [ ] Verify SHA-256 of all artifacts before copying them.
- [ ] Stop a pre-existing current-user task, copy files atomically, register/start the task, and poll `http://127.0.0.1:43127/health`.
- [ ] Make uninstallation remove only plugin/runtime/task paths, retaining recordings and protected credentials unless explicitly requested.
- [ ] Test installation in a disposable per-user directory and assert no path targets Program Files or system-wide locations.

### Task 4: Build a reproducible Windows Codex bundle in CI

**Files:**
- Create: `.github/workflows/windows.yml`
- Modify: `.github/workflows/release.yml`
- Create: `scripts/build-windows-bundle.ps1`
- Create: `scripts/verify-windows-bundle.ps1`

**Interfaces:**
- Produces `OBS-Telegram-Send-Windows-x64-Codex.zip` containing only the four installer inputs from Task 3.
- Fails if architecture is not x64, artifacts are missing, checksums do not match, or the plugin cannot be loaded by the supported OBS build.

- [ ] Pin OBS 32.1.1 x64 archive/checksum and the exact Telegram Bot API source revision.
- [ ] Build the Rust agent for `x86_64-pc-windows-msvc` and compile the Bot API with MSVC/CMake/Vcpkg dependencies.
- [ ] Build the plugin, run C++ and Rust tests, create checksums, and run installer layout validation.
- [ ] Upload a development artifact first; do not publish a release until a Windows runtime test succeeds.

### Task 5: Add the client-facing Codex installation flow

**Files:**
- Modify: `README.md`
- Create: `docs/installar-com-codex-pt-BR.md`
- Modify: `docs/onboarding-pt-BR.md`
- Modify: `docs/troubleshooting-pt-BR.md`

**Interfaces:**
- Produces one copy-and-paste prompt each for macOS and Windows.
- The Windows prompt instructs Codex to verify the public release checksum, run the local PowerShell installer, open OBS, and guide onboarding.

- [ ] State clearly that clients need Codex on their computer and that Windows SmartScreen may require an informed approval for this unsigned public bundle.
- [ ] Never ask clients to paste Telegram secrets into chat; secrets are entered only into OBS onboarding fields.
- [ ] Link each operating system to the correct release asset and cleanup procedure.

## Self-review

- [ ] The plan preserves the original recording confirmation and manual-send behavior.
- [ ] Every Windows-installed executable is x64 and checksum-verified.
- [ ] No secret reaches GitHub Actions artifacts, PowerShell transcripts, or documentation examples.
- [ ] Windows packaging is tested before its release asset is published.
