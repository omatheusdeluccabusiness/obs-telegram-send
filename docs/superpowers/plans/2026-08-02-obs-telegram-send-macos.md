# OBS Telegram Send macOS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and package a macOS OBS plugin and local companion that lets a creator explicitly approve sending a completed recording to their own Telegram chat in original quality up to 2 GB.

**Architecture:** A C++/Qt OBS module owns the onboarding and central post-recording confirmation dialog. A Rust loopback companion owns credentials, starts the official local Telegram Bot API server, receives explicit send requests, and streams the completed file to it. The plugin and companion exchange versioned JSON messages only over `127.0.0.1`.

**Tech Stack:** C++17, Qt supplied by OBS, OBS Studio 32.1.1 Frontend API, CMake, Rust stable, Tokio, Reqwest, Serde, keyring, official Telegram Bot API Server, GitHub Actions and GitHub Releases.

## Global Constraints

- The first release targets macOS and must be tested against OBS Studio 32.1.1 installed at `/Applications/OBS.app`.
- The public project is separate from `obs-gancho-visual` and is named `obs-telegram-send`.
- No recording is sent unless the user has explicitly checked **Enviar este vídeo ao Telegram** in the central OBS dialog for that specific completed recording.
- The confirmation checkbox is unchecked on every new recording; **Enviar agora** stays disabled until it is checked.
- Credentials are stored only in macOS Keychain; no token, `api_id`, `api_hash`, or absolute user file path appears in logs, tests, commits, scene collections, or documentation examples.
- The companion binds only to `127.0.0.1`, accepts only a per-install generated bearer secret, and never deletes recordings.
- The official local Bot API server is invoked with `--local`; jobs above 2 GB are rejected before upload with a clear user-facing message.
- Onboarding is Portuguese (Brazil), written for non-technical users, and includes an in-app numbered guide plus direct links to BotFather and `my.telegram.org`.

---

## File Structure

| Path | Responsibility |
| --- | --- |
| `CMakeLists.txt` | Top-level plugin build and packaging configuration. |
| `plugin/src/module.cpp` | OBS module entry points and frontend event registration. |
| `plugin/src/recording_controller.{hpp,cpp}` | Resolve a completed recording and enforce explicit confirmation before dispatch. |
| `plugin/src/send_confirmation_dialog.{hpp,cpp}` | Central Qt dialog with checkbox, metadata, status and retry actions. |
| `plugin/src/onboarding_dialog.{hpp,cpp}` | Portuguese setup wizard, chat detection and test-send controls. |
| `plugin/src/agent_client.{hpp,cpp}` | Authenticated loopback JSON client. |
| `plugin/tests/*.cpp` | Plugin unit tests using a fake agent client. |
| `agent/Cargo.toml` | Companion dependencies and binary definition. |
| `agent/src/main.rs` | Loopback service startup and lifecycle. |
| `agent/src/api.rs` | Versioned local HTTP routes and bearer middleware. |
| `agent/src/config.rs` | Validated configuration shapes without secret persistence. |
| `agent/src/keychain.rs` | macOS Keychain reads/writes. |
| `agent/src/telegram.rs` | Start local Bot API server, detect chat, test and upload jobs. |
| `agent/src/jobs.rs` | Persistent non-secret job state, progress and explicit retry. |
| `agent/tests/*.rs` | Unit and integration tests with a fake Telegram endpoint. |
| `installer/macos/*` | Signed/notarized package layout, launch agent and uninstall script. |
| `docs/onboarding-pt-BR.md` | Standalone accessible version of the in-app onboarding. |
| `.github/workflows/{test,release}.yml` | Test/build and tagged release automation. |

## Local Protocol

The plugin sends `POST /v1/jobs` with `{"recording_path":"<absolute path>","display_name":"<filename>"}` only after dialog confirmation. The companion returns `{"job_id":"uuid","state":"queued"}`. The plugin polls `GET /v1/jobs/{job_id}` for `queued`, `uploading`, `completed` or `failed` plus `progress_percent` and a safe `message`.

The companion receives onboarding routes: `POST /v1/config`, `POST /v1/chat/detect`, and `POST /v1/test-send`. Each request must include `Authorization: Bearer <per-install secret>`; the bearer secret is created by the installer and stored in Keychain alongside the account configuration.

### Task 1: Bootstrap the repository and test harness

**Files:**
- Create: `CMakeLists.txt`, `plugin/CMakeLists.txt`, `plugin/src/module.cpp`, `plugin/tests/CMakeLists.txt`, `README.md`, `.gitignore`
- Create: `agent/Cargo.toml`, `agent/src/main.rs`, `agent/tests/health_test.rs`

**Interfaces:**
- Produces: CMake target `obs-telegram-send` and Rust binary `obs-telegram-agent`.
- Produces: agent route `GET /health` returning `{"version":"0.1.0","status":"ok"}`.

- [ ] **Step 1: Write the failing health test**

```rust
#[tokio::test]
async fn health_returns_the_current_protocol_version() {
    let response = app().oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["version"], "0.1.0");
}
```

- [ ] **Step 2: Run the test and verify it fails because `app` is absent**

Run: `cargo test --manifest-path agent/Cargo.toml health_returns_the_current_protocol_version`

- [ ] **Step 3: Implement the smallest Axum app and binary startup**

```rust
pub fn app() -> Router {
    Router::new().route("/health", get(|| async { Json(json!({"version":"0.1.0","status":"ok"})) }))
}
```

- [ ] **Step 4: Re-run the agent test and configure the CMake plugin module target**

Run: `cargo test --manifest-path agent/Cargo.toml && cmake -S . -B build -DOBS_APP_BUNDLE=/Applications/OBS.app`

- [ ] **Step 5: Commit**

```bash
git add CMakeLists.txt plugin agent README.md .gitignore
git commit -m "feat: bootstrap OBS Telegram Send"
```

### Task 2: Add safe configuration and Keychain storage

**Files:**
- Create: `agent/src/config.rs`, `agent/src/keychain.rs`, `agent/tests/config_test.rs`
- Modify: `agent/src/main.rs`

**Interfaces:**
- Produces: `TelegramConfig { bot_token: SecretString, api_id: u32, api_hash: SecretString, chat_id: i64 }`.
- Produces: `validate_config(input: ConfigInput) -> Result<TelegramConfig, ConfigError>`.
- Produces: `SecretStore::save`, `SecretStore::load`, `SecretStore::clear`.

- [ ] **Step 1: Write failing validation tests**

```rust
#[test]
fn rejects_a_token_without_the_botfather_separator() {
    assert_eq!(validate_config(input("not-a-token")).unwrap_err(), ConfigError::InvalidBotToken);
}

#[test]
fn accepts_a_private_chat_identifier() {
    assert_eq!(validate_config(input("123:abc", 12345)).unwrap().chat_id, 12345);
}
```

- [ ] **Step 2: Verify tests fail because configuration types do not exist**

Run: `cargo test --manifest-path agent/Cargo.toml config_test`

- [ ] **Step 3: Implement validation and a Keychain-backed `SecretStore`**

```rust
pub fn validate_config(input: ConfigInput) -> Result<TelegramConfig, ConfigError> {
    if !input.bot_token.contains(':') { return Err(ConfigError::InvalidBotToken); }
    if input.api_id == 0 || input.api_hash.trim().is_empty() { return Err(ConfigError::InvalidTelegramApp); }
    Ok(TelegramConfig::from(input))
}
```

- [ ] **Step 4: Re-run all agent tests and confirm no secret is rendered in errors**

Run: `cargo test --manifest-path agent/Cargo.toml`

- [ ] **Step 5: Commit**

```bash
git add agent
git commit -m "feat: store Telegram configuration securely"
```

### Task 3: Implement the authenticated loopback protocol

**Files:**
- Create: `agent/src/api.rs`, `agent/tests/auth_test.rs`
- Modify: `agent/src/main.rs`, `agent/src/config.rs`

**Interfaces:**
- Consumes: `SecretStore` from Task 2.
- Produces: `POST /v1/config` and bearer-authenticated middleware.
- Produces: `AgentError { code: String, message: String }` with no secret fields.

- [ ] **Step 1: Write failing authentication tests**

```rust
#[tokio::test]
async fn configuration_is_rejected_without_the_install_bearer() {
    let response = app_with_secret("install-secret").oneshot(post_json("/v1/config", valid_input())).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 2: Verify the route test fails because the route is absent**

Run: `cargo test --manifest-path agent/Cargo.toml configuration_is_rejected_without_the_install_bearer`

- [ ] **Step 3: Implement bearer middleware and configuration route**

```rust
async fn require_bearer(State(secret): State<String>, request: Request, next: Next) -> Response {
    if request.headers().get(AUTHORIZATION) == Some(&HeaderValue::from_str(&format!("Bearer {secret}")).unwrap()) {
        return next.run(request).await;
    }
    (StatusCode::UNAUTHORIZED, Json(AgentError::unauthorized())).into_response()
}
```

- [ ] **Step 4: Run all loopback tests and bind the service to `127.0.0.1` only**

Run: `cargo test --manifest-path agent/Cargo.toml && lsof -nP -iTCP:43127 -sTCP:LISTEN`

- [ ] **Step 5: Commit**

```bash
git add agent
git commit -m "feat: add authenticated local agent API"
```

### Task 4: Add local Bot API lifecycle, chat detection and safe upload jobs

**Files:**
- Create: `agent/src/telegram.rs`, `agent/src/jobs.rs`, `agent/tests/jobs_test.rs`, `agent/tests/telegram_test.rs`
- Modify: `agent/src/api.rs`, `agent/src/main.rs`

**Interfaces:**
- Produces: `TelegramGateway::{detect_chat,test_send,upload_file}`.
- Produces: `JobService::{enqueue,status,retry}`.
- Produces: `POST /v1/jobs`, `GET /v1/jobs/{id}`, `POST /v1/jobs/{id}/retry`, `POST /v1/chat/detect`, `POST /v1/test-send`.

- [ ] **Step 1: Write failing consent and size tests**

```rust
#[test]
fn rejects_a_file_larger_than_two_gibibytes_before_upload() {
    assert_eq!(JobService::validate_size(2 * 1024_u64.pow(3) + 1), Err(JobError::FileTooLarge));
}

#[tokio::test]
async fn enqueue_never_calls_telegram_until_the_plugin_posts_a_job() {
    let gateway = FakeGateway::default();
    let service = JobService::new(gateway.clone());
    assert!(gateway.uploads().is_empty());
    assert!(service.jobs().is_empty());
}
```

- [ ] **Step 2: Run the tests and verify the service does not yet exist**

Run: `cargo test --manifest-path agent/Cargo.toml jobs_test`

- [ ] **Step 3: Implement the local-server launcher and job service**

```rust
pub async fn enqueue(&self, path: PathBuf, display_name: String) -> Result<JobId, JobError> {
    let size = tokio::fs::metadata(&path).await?.len();
    Self::validate_size(size)?;
    self.persist_queued(path, display_name).await
}
```

Start `telegram-bot-api --local --http-port <loopback-port>` with `TELEGRAM_API_ID` and `TELEGRAM_API_HASH` supplied only in the child process environment, then send `sendVideo` for MP4 or `sendDocument` otherwise. Store only job id, filename, state, size and retryable error; never persist the absolute recording path after a completed job.

- [ ] **Step 4: Run unit and fake-endpoint integration tests**

Run: `cargo test --manifest-path agent/Cargo.toml`

- [ ] **Step 5: Commit**

```bash
git add agent
git commit -m "feat: add Telegram upload queue"
```

### Task 5: Build the native confirmation flow in OBS

**Files:**
- Create: `plugin/src/recording_controller.hpp`, `plugin/src/recording_controller.cpp`, `plugin/src/send_confirmation_dialog.hpp`, `plugin/src/send_confirmation_dialog.cpp`, `plugin/tests/recording_controller_test.cpp`
- Modify: `plugin/src/module.cpp`, `plugin/CMakeLists.txt`

**Interfaces:**
- Consumes: `AgentClient::create_job(path, display_name)` from Task 6.
- Produces: `RecordingController::on_frontend_event(obs_frontend_event)`.
- Produces: `SendConfirmationDialog::show_for(RecordingMetadata)`.

- [ ] **Step 1: Write the failing consent test**

```cpp
TEST(RecordingController, DoesNotCreateUploadBeforeCheckboxConfirmation) {
  FakeAgentClient agent;
  RecordingController controller(agent);
  controller.recording_finished(metadata("take.mp4", 123));
  EXPECT_TRUE(agent.created_jobs().empty());
}
```

- [ ] **Step 2: Run the plugin test and verify it fails because `RecordingController` is absent**

Run: `cmake --build build --target plugin-tests && ./build/plugin/tests/plugin-tests --gtest_filter=RecordingController.*`

- [ ] **Step 3: Implement central dialog and controller**

```cpp
connect(sendButton, &QPushButton::clicked, this, [this] {
  if (!consentCheckBox->isChecked()) return;
  emit sendConfirmed(metadata_);
});
sendButton->setEnabled(false);
connect(consentCheckBox, &QCheckBox::toggled, sendButton, &QPushButton::setEnabled);
```

Register only `OBS_FRONTEND_EVENT_RECORDING_STOPPED`; resolve the completed path through the OBS frontend output API and keep the dialog modal to the OBS main window.

- [ ] **Step 4: Run plugin tests and manually check the central dialog in OBS**

Run: `cmake --build build --target plugin-tests && ctest --test-dir build --output-on-failure`

- [ ] **Step 5: Commit**

```bash
git add plugin CMakeLists.txt
git commit -m "feat: add manual recording send confirmation"
```

### Task 6: Connect the OBS onboarding and job status UI

**Files:**
- Create: `plugin/src/agent_client.hpp`, `plugin/src/agent_client.cpp`, `plugin/src/onboarding_dialog.hpp`, `plugin/src/onboarding_dialog.cpp`, `plugin/tests/agent_client_test.cpp`
- Modify: `plugin/src/module.cpp`, `plugin/src/send_confirmation_dialog.cpp`

**Interfaces:**
- Consumes: all `/v1/*` routes from Tasks 3 and 4.
- Produces: `AgentClient::{save_config,detect_chat,test_send,create_job,get_job,retry_job}`.
- Produces: menu entry **Ferramentas → Telegram Send**.

- [ ] **Step 1: Write failing client and UI-state tests**

```cpp
TEST(AgentClient, NeverIncludesBearerInVisibleError) {
  auto result = client_for("install-secret").parse_error("{\"message\":\"invalid\"}");
  EXPECT_EQ(result.message, "invalid");
  EXPECT_FALSE(result.message.contains("install-secret"));
}

TEST(SendConfirmationDialog, SendButtonStartsDisabledForEveryRecording) {
  SendConfirmationDialog dialog(fakeAgent());
  dialog.show_for(metadata("new-take.mp4", 42));
  EXPECT_FALSE(dialog.send_button_enabled());
}
```

- [ ] **Step 2: Run the tests and verify they fail before client/UI implementation**

Run: `cmake --build build --target plugin-tests && ./build/plugin/tests/plugin-tests --gtest_filter='AgentClient.*:SendConfirmationDialog.*'`

- [ ] **Step 3: Implement Portuguese wizard and status polling**

The wizard pages must be titled **Seu bot**, **Seu acesso ao Telegram**, **Seu chat** and **Teste final**. Include direct buttons to `https://t.me/BotFather` and `https://my.telegram.org`, plain-language instructions, a **Detectar meu chat** action after the `/start` instruction, and a **Enviar mensagem de teste** action. Poll job status every 500 ms while uploading and expose only filename, percentage and safe agent messages.

- [ ] **Step 4: Run all C++ and Rust tests**

Run: `ctest --test-dir build --output-on-failure && cargo test --manifest-path agent/Cargo.toml`

- [ ] **Step 5: Commit**

```bash
git add plugin agent
git commit -m "feat: add Telegram onboarding and upload status"
```

### Task 7: Package the macOS release and document the client journey

**Files:**
- Create: `installer/macos/build-package.sh`, `installer/macos/uninstall.sh`, `installer/macos/LaunchAgent.plist`, `docs/onboarding-pt-BR.md`, `docs/troubleshooting-pt-BR.md`, `LICENSE`
- Modify: `README.md`, `CMakeLists.txt`

**Interfaces:**
- Consumes: built `obs-telegram-send.plugin`, `obs-telegram-agent`, and official `telegram-bot-api` binary.
- Produces: `dist/OBS-Telegram-Send-macOS.pkg` and a launch agent that starts only the companion loopback service.

- [ ] **Step 1: Write failing package-layout test**

```bash
test -x dist/root/Library/Application\ Support/obs-studio/plugins/obs-telegram-send.plugin/Contents/MacOS/obs-telegram-send
test -x dist/root/Library/Application\ Support/OBS-Telegram-Send/obs-telegram-agent
test -x dist/root/Library/Application\ Support/OBS-Telegram-Send/telegram-bot-api
```

- [ ] **Step 2: Run it and verify it fails because the package has not been assembled**

Run: `bash installer/macos/package-layout-test.sh`

- [ ] **Step 3: Implement package build, uninstall and documentation**

The installer must copy files without credentials, generate its loopback bearer secret at first launch, register the launch agent, and request no network permission until configuration is explicitly saved. Documentation must use screenshots captured from the completed product before release.

- [ ] **Step 4: Build and inspect a package**

Run: `bash installer/macos/build-package.sh && pkgutil --check-signature dist/OBS-Telegram-Send-macOS.pkg`

- [ ] **Step 5: Commit**

```bash
git add installer docs README.md LICENSE CMakeLists.txt
git commit -m "feat: package macOS OBS Telegram Send"
```

### Task 8: Add repeatable CI and release checks

**Files:**
- Create: `.github/workflows/test.yml`, `.github/workflows/release.yml`, `scripts/verify-release.sh`

**Interfaces:**
- Consumes: CMake plugin build, Cargo test suite, package layout from Task 7.
- Produces: GitHub release asset when a signed `v*` tag is pushed.

- [ ] **Step 1: Write failing verification script checks**

```bash
set -euo pipefail
cargo test --manifest-path agent/Cargo.toml
ctest --test-dir build --output-on-failure
test -f dist/OBS-Telegram-Send-macOS.pkg
```

- [ ] **Step 2: Run the script and verify it fails before CI/package configuration exists**

Run: `bash scripts/verify-release.sh`

- [ ] **Step 3: Implement GitHub Actions workflows**

Run tests on every pull request using macOS, build the unsigned artifact on tagged releases, and upload it as a draft release asset. Keep signing/notarization steps conditional on repository secrets; a release cannot be marked public until a signed package has been manually verified on a clean macOS account.

- [ ] **Step 4: Run the release verification locally**

Run: `bash scripts/verify-release.sh`

- [ ] **Step 5: Commit**

```bash
git add .github scripts
git commit -m "ci: verify OBS Telegram Send releases"
```

## Plan Self-Review

- Spec coverage: onboarding (Task 6 and 7), consent dialog (Task 5), secure local service (Tasks 2–4), quality/original file and 2 GB limit (Task 4), packaging (Task 7), tests and release checks (Tasks 1–8).
- Scope: macOS is a complete vertical slice. Windows is deliberately a separate follow-up plan after the local service protocol and UI have been proved on macOS.
- Security: tokens are constrained to Keychain, loopback is authenticated, paths are never logged, and sends require explicit UI confirmation.
- Type consistency: `AgentClient` consumes the exact HTTP contract defined in Local Protocol; `JobService` is the only producer of job states polled by the dialog.
