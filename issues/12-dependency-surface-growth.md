# 12 - Dependency surface roughly doubled

Status: open
Severity: low
Introduced: `feature/beet-pusher-web`

## Observation

`Cargo.lock` grew by ~930 lines on this branch. New direct dependencies, all via
`beet-pusher-webui`: `axum`, `tower`, `tokio`, `utoipa`, `utoipa-swagger-ui`,
`validator`, `config`, `tracing-subscriber` (json feature).

This was handled responsibly — `supply-chain/audits.toml` (+452 lines),
`config.toml`, and `imports.lock` were all updated to match, and `cargo-vet` was bumped
via npins. This issue is a note to revisit, not a complaint.

## Points to revisit

- **`utoipa-swagger-ui` with `vendored`** — the swagger-ui frontend assets are compiled
  into the binary. Convenient for development; worth deciding whether the release build
  should carry them, or whether the whole swagger route should sit behind a feature flag
  (which would also address part of [07](07-webui-has-no-authentication.md)).
- **`validator`** — currently earns very little. `CreateBucketDto`
  (`src/api/handlers/node.rs:24-27`) derives `Validate` but declares no rules; the actual
  validation is the `parent.parse()` call in the handler. Either use it properly or drop
  it and the `ValidatedJson` extractor in favour of plain `Json` plus explicit parsing.
- **`config`** — one struct with two fields
  (`crates/beet-pusher-webui/src/config.rs`). The workspace already uses
  `arg_util::ConfigFileOpen` and `toml` for exactly this job in `beet-pusher`. Worth
  asking whether a second config mechanism is warranted; see also
  [06](06-config-reads-unprefixed-env.md).
- **`tokio` alongside the existing sync/thread model** — the webui is async, the backend
  is threads-and-channels, and `StdioPipe` bridges them
  (`blocking_recv`/`blocking_send` inside `std::thread::spawn`). That bridge is fine but
  is a genuine complexity boundary; worth keeping the async surface confined to the webui
  crate.

## Action

No immediate change required. Re-evaluate `validator` and `config` when the second and
third endpoints land — that is the point at which their value (or lack of it) becomes
clear.
