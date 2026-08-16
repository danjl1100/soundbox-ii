# 03 - Any VLC HTTP error terminates the `beet-pusher` daemon

Status: open
Severity: high
Introduced: `feature/beet-pusher-web`

## Problem

In the main event loop, `crates/beet-pusher/src/bin/beet-pusher.rs:189-192`:

```rust
let hint_need_fill = pusher.push_playlist_update(
    &mut http_runner,
    Some(&mut now_playing_observer),
)?;
```

The `?` propagates straight out of `main`. Any error from the VLC HTTP request — VLC not
yet started, restarted, a dropped connection, a transient network blip — exits the
process.

## Why it matters

Before this branch, `beet-pusher` was a short-lived foreground tool where exiting on
error was reasonable. It is now the backend half of a long-running service: when it
exits, its stdout closes, the webui's stdin thread sees EOF and triggers shutdown
(`crates/beet-pusher-webui/src/infra/stdio_pipe.rs:99`), and the whole stack goes down.
A user restarting VLC should not take the web UI with it.

## Fix

[x] Handle the error inside the loop rather than propagating: log it, and either retry with
backoff or defer the timer and continue. `timer_fill_playlist.defer()` already exists as
the "try again later" mechanism.

[x] Decide deliberately which errors *are* fatal (e.g. bad auth credentials, which will never
succeed on retry) and keep only those propagating — the `Error`/`ErrorKind` split in
`crates/beet-pusher/src/pusher.rs:365+` gives a place to make that distinction explicit.

The same question applies to `fill_buckets(&mut beet_cmd, spigot)?` at
`bin/beet-pusher.rs:211`, where a `beet` invocation failure is likewise fatal.
