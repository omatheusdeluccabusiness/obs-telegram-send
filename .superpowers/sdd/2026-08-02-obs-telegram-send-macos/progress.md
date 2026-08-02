# SDD ledger — plan: docs/superpowers/plans/2026-08-02-obs-telegram-send-macos.md

Merge base: 0705301
Plan review: credential values are passed to the official local Bot API child through environment variables instead of command-line arguments.
Task 1: complete (commits e161327..1f48bb0, review clean)
Task 1: minor (deferred): ephemeral test-port reservation has a small startup race; liveness check prevents a false positive.
Task 2: complete (commits 1f48bb0..59156a8, review clean)
Task 2: minor (deferred): report used a cargo test name filter that ran 0 tests; full suite ran the 10 configuration cases.
Task 3: complete (commits 59156a8..c23aff1, review clean)
Task 4: parked — same-UID hostile-process TCP rebind remains theoretically possible with the official TCP-only local Bot API server; ruling: first-release threat model is loopback-only service, ephemeral port and protected credentials, per user direction not to expand scope.
Task 4: complete (commits c23aff1..5e1188f, 1 parked)
Task 5: complete (commits 5e1188f..3eb608f, review clean)
Task 6: complete (commits 3eb608f..cac9d27, review clean after 3 fix rounds)
Task 7: complete (commits cac9d27..3dfa71f, review clean after 2 fix rounds)
Task 7: release gates active — public notarized package remains blocked by missing Developer ID identities, four real screenshots and Codex-injected AppleDouble metadata; development package is explicitly non-publicable.
Task 8: complete (commits 3dfa71f..0c173e7, review clean after 2 fix rounds)
Task 8: GitHub Environment `release` created with required reviewer 272657693 and tag-only policy `v*`.
Task 7 runtime re-review: `ba8c78b` clean; targeted 3/3, full agent 45/45, release build and development package passed.
Runtime repair: separated the packaged `telegram-bot-api` executable from `telegram-bot-api-data`; regression tests cover the former file/directory collision.
Runtime repair: changed Axum 0.7 dynamic job routes to `/:job_id`; real loopback GET now returns HTTP 200 and the completed job state.
Runtime repair: established the first public Keychain namespace `com.obs-telegram-send.agent.v1`, leaving the obsolete ad-hoc development ACL untouched.
End-to-end macOS validation: onboarding detected the real Reels Assistant IA chat, the test message appeared in Telegram, and `2026-08-02 20-25-36.mov` uploaded to 100% with 5,053,651 bytes while the original remained in `~/Movies`.
Final local suites: Rust 46/46, strict clippy, release build, C++ 20/20, development package layout pass. Public release remains fail-closed without Developer ID/notarization and the two remaining real release screenshots.
