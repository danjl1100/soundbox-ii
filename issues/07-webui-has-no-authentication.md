# 07 - No authentication on the webui control surface

Status: open
Severity: medium
Introduced: `feature/beet-pusher-web`

## Problem

`crates/beet-pusher-webui` binds a TCP listener and serves routes that mutate the bucket
spigot and control VLC playback, with no authentication or authorisation anywhere in the
request path (`src/routes.rs:12-22`, `src/api/handlers/node.rs`).

`AppError::Unauthorized` and `AppError::Forbidden` exist with correct status mappings
(`src/error.rs:20-22`, `58-62`) but are never constructed — placeholders, not enforcement.

`bind_ip` is free-form (`src/config.rs:7`), so binding to `0.0.0.0` is a one-variable
change away, and the swagger-ui at `/swagger-ui` publishes the full API surface to
whoever can reach it.

## Why it matters

Not urgent while everything is loopback-only development, but this needs a deliberate
decision *before* anything binds beyond `127.0.0.1` — and defaults have a way of becoming
permanent. `vlc-http-auth` already exists in the workspace as prior art for how this
project handles credentials.

## Fix

Decide the intended threat model and record it. Options, roughly in order of effort:

1. Document loopback-only as an intentional constraint, and reject non-loopback `bind_ip`
   values in `Config::validate` until auth exists.
2. Shared-secret bearer token via an axum middleware layer, mirroring the bearer scheme
   `fake-vlc` already validates (`crates/fake-vlc/src/lib.rs`, `auth_result` module).
3. Full session auth, if the UI ever grows real multi-user needs.

Option 1 is cheap and makes the constraint enforced rather than assumed; it can land with
this work and be superseded later.
