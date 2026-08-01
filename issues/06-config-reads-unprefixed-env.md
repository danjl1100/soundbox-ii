# 06 - webui config reads the unprefixed environment

Status: open
Severity: medium
Introduced: `feature/beet-pusher-web`

## Problem

`crates/beet-pusher-webui/src/config.rs:15-28`:

```rust
let config = ::config::Config::builder()
    .add_source(::config::Environment::default().separator("__"))
    .build()?;
```

`Environment::default()` has no prefix, so the fields `bind_ip` and `port` are populated
from bare `BIND_IP` and `PORT` environment variables. Both are required, with no
defaults, so the process refuses to start without them.

## Why it matters

`PORT` is one of the most commonly set ambient variables there is — set by process
managers, CI runners, and shells. An unrelated `PORT` in the environment silently
redirects where the service binds. The blast radius grows with every field added to
`Config`.

Everything else on this branch already namespaces its environment
(`BEET_PUSHER_CONFIG_FILE`, `BEET_PUSHER_WEBUI`, `BEET_PUSHER_BACKEND`,
`FAKE_BEET_CONFIG_FILE`), so this is also inconsistent.

## Fix

Use `Environment::with_prefix("BEET_PUSHER_WEBUI").separator("__")`, and supply sensible
defaults (`127.0.0.1`, and a fixed port) via `set_default` so the binary runs with no
configuration at all.

Update the two callers that set the current names:

- `crates/xtask/src/beet_pusher.rs:125-126` (`BIND_IP`, `PORT`)
- `crates/beet-pusher-webui/tests/common/end_to_end/pipe_runner.rs:22-25`

Note the separate `SCRIPT_WRITE_PORT` variable read directly via `std::env::var` at
`crates/beet-pusher-webui/src/bin/beet-pusher-webui.rs:33` (and `LOG_JSON` at
`src/lib.rs:30`) — these bypass `Config` entirely and would ideally be folded into it.

## Resolution

Added prefix to env vars for `beet_pusher_webui`.
