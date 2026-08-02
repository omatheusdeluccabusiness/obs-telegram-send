# OBS Telegram Send

OBS Telegram Send is a macOS OBS Studio plugin and a local companion service
for an explicitly approved Telegram upload after a recording finishes.

## Development bootstrap

Requirements:

- macOS with OBS Studio 32.1.1 installed in `/Applications/OBS.app`
- CMake 3.28 or newer
- Rust stable
- OBS development headers compatible with OBS Studio 32.1.1 for compiling the
  native module

Run the test suite and configure the native target:

```sh
cargo test --manifest-path agent/Cargo.toml
cmake -S . -B build -DOBS_APP_BUNDLE=/Applications/OBS.app
```

The local agent exposes `GET /health`, which returns protocol version `0.1.0`
and status `ok`. It listens only on the loopback interface. Uploading,
credentials, and production send behavior are intentionally not implemented in
this bootstrap.
