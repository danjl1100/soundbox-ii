# 04 - A single malformed stdout line shuts down the webui

Status: open
Severity: medium
Introduced: `feature/beet-pusher-web`

## Problem

In the webui's stdin reader thread,
`crates/beet-pusher-webui/src/infra/stdio_pipe.rs:77`:

```rust
let response: beet_pusher::pipe_exec::ResponseOutDe = serde_json::from_str(&line)?;
```

The `?` exits the `for line in stdin().lines()` loop, which falls through to
`shutdown_tx.blocking_send(Shutdown)` at line 99 — terminating the HTTP server.

## Why it matters

The entire protocol depends on `beet-pusher` (and every crate it links, now and in the
future) never writing a single stray line to stdout. Today that holds: tracing goes to
stderr (`bin/beet-pusher.rs:359`), `now_playing_observer` uses `eprintln!`, and only
`pipe_cmd_loop` uses `println!`. But it is an invariant with no enforcement and a very
harsh failure mode — one `dbg!` or a dependency that logs to stdout takes the service
down, with the cause several layers removed from the symptom.

## Fix

Log the parse failure and continue to the next line rather than exiting. Reserve shutdown
for genuine EOF (the loop ending naturally) and for `io::Error` on the read itself.

Consider also whether the protocol should be framed more defensively — e.g. a required
sentinel prefix on protocol lines so non-protocol output is unambiguously skippable.

## Related

Same file and layer as [05](05-waiting-channels-leak.md); worth fixing together.
