# 15 - Slow `beet` and VLC work runs on the command-loop thread

Status: open
Severity: medium
Introduced: `feature/beet-pusher-web`

Split out of [02](closed/02-pipe-timeout-mismatch.md) ("Related" section), which fixed the
mismatched timeout constants but left the reason the budget gets blown in the first place.

## Problem

`CommandLoop::run` (`crates/beet-pusher/src/command_loop.rs:94-181`) is a single thread
doing three jobs, in a fixed priority ladder:

```rust
let event = if timer_fill_playlist.is_ready() {
    Ok(LoopEvent::MaintainPlaylist)      // blocking HTTP to VLC
} else if timer_fill_bucket.is_ready() {
    Ok(LoopEvent::MaintainBuckets)       // blocking `beet` subprocess, per bucket
} else {
    loop_rx.recv_timeout(TICK_INTERVAL)  // client commands, last
};
```

Both of the higher-priority arms block, without bound:

- `MaintainPlaylist` calls `pusher.push_playlist_update(&mut http_runner, ..)`
  (`command_loop.rs:113`), which issues synchronous HTTP to VLC over
  `vlc_http_ureq::HttpRunner`. A slow or wedged VLC stalls the thread for the duration of
  the request.
- `MaintainBuckets` calls `fill_buckets` (`command_loop.rs:135`), which loops over
  *every* bucket needing fill and runs one `beet` query each
  (`crates/beet-pusher/src/beet.rs:47-57` → `crates/beet-pusher/src/beet/query.rs:78-85`,
  `Command::output()`). On a real library this is seconds per query, times the number of
  pending buckets, all before the loop looks at `loop_rx` again.

Client commands are dequeued only in the `else` branch, so they wait behind whichever of
those is in flight.

## Why it matters

This is what makes the pipe protocol's timeouts unreliable, and 02's fix does not reach it.
`pipe_cmd` (`command_loop.rs:332-357`) enqueues the event and waits `RESPONSE_TIMEOUT`
(500 ms); on expiry it replies `ErrorKind::InternalTimeout` **but leaves the event in the
channel**, so the loop executes it once it gets back around. The webui reports a failure for
a command that then succeeds. `SpigotCmd::AddNode` is not idempotent, so a retry creates a
second bucket — the exact silent state corruption 02 was filed for, now reachable only via
contention instead of on every request.

Two smaller consequences of the same shape:

- `loop_tx.as_ref().send(event)` (`command_loop.rs:348`) is a `sync_channel(1)` send with no
  timeout at all. With one command already queued, a second pipe command blocks the pipe
  thread indefinitely, before its own 500 ms budget starts.
- Any raise of the client timeout to cover a worst-case `fill_buckets` would have to be on
  the order of seconds, which is not a usable latency budget for an interactive webui. The
  timeout cannot be tuned out of this; the blocking work has to move.

It also bounds what the webui can grow into: every endpoint added inherits the worst-case
latency of a full library refill.

## Fix

Get the unbounded work off the thread that answers commands. Roughly in order of cost:

1. [x] Run `beet` queries on a worker thread (or a small pool) and deliver results back to the
   loop as a `LoopEvent::BucketsFilled { bucket, new_contents }`. `fill_buckets` already
   separates "which buckets need fill" from "query" from "apply"
   (`beet.rs:42-70`), so the split is mostly mechanical: the loop keeps ownership of the
   `Network`, the worker only owns the `BeetRunner` and returns items.
2. [x] Do the same for the VLC round trip, or at minimum give `HttpRunner` a request timeout so
   `MaintainPlaylist` has a bounded worst case. Note [03](03-vlc-error-terminates-daemon.md)
   is about the *error* path of this same call — worth doing together.
3. [x] Reconsider the strict priority ladder once the blocking work is gone. With both slow arms
   asynchronous, commands can be served promptly without starving maintenance, and
   `TICK_INTERVAL` stops being load-bearing for command latency.
4. [ ] Give the `loop_tx.send` in `pipe_cmd` a timeout, so a full channel surfaces as an error
   rather than an unbounded stall.

## Related

Fixing this shrinks the window in which a client-side timeout races a command that still
executes — it does not close it. Closing it needs the response to be replayable or the
command to be idempotent: a reply cache keyed by `RequestSequence`, and/or the
fully-specified commands proposed in
[14](14-fully-specified-add-commands.md). This issue and 14 are complements, not
alternatives.

[13](13-webui-architecture-review.md) covers the layering this would sit inside;
[05](05-waiting-channels-leak.md) and [04](04-stdin-thread-dies-on-bad-line.md) are the same
protocol layer.
