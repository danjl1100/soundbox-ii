# 02 - webui/backend pipe timeouts are mismatched, and retries are not idempotent

Status: closed
Severity: high
Introduced: `feature/beet-pusher-web`

## Problem

Two unrelated timeout constants in two crates govern the same round trip:

- `crates/beet-pusher-webui/src/domain/services/node_service.rs:31`
  (used to be: `const TIMEOUT: Duration = from_millis(100)`) — how long the
  webui waits for a reply.
- `crates/beet-pusher/src/command_loop.rs:315`
  `const RESPONSE_TIMEOUT: Duration = from_millis(500)` — how long the backend's pipe
  thread waits for its own event loop.

The caller's budget is five times shorter than the callee's. Worse, the backend event
loop (`bin/beet-pusher.rs:170-257`) only reaches `loop_rx.recv_timeout(TICK_INTERVAL)`
after checking two timers, and `TICK_INTERVAL` is itself 100 ms — so the webui's entire
budget can be consumed by the poll interval before the command is even dequeued. If
`MaintainPlaylist` fires first it performs blocking HTTP to VLC, consuming more.

## Why it matters

The webui reports a failure while the backend goes on to execute the command anyway.
`SpigotCmd::AddNode` is not idempotent, so a user (or client) who retries a "failed"
create-bucket ends up with two buckets. This is silent, state-corrupting, and will get
more likely as endpoints multiply.

The end-to-end test `times_out` in
`crates/beet-pusher-webui/tests/common/end_to_end.rs:33` currently *asserts* this
behaviour, so it will need updating alongside the fix.

## Fix

- [x] Make the webui timeout at least as long as the backend's, so a timeout on the caller
   side implies the callee has also given up.
- [x] Lift both from hard-coded consts to configuration, with a documented relationship
   (`webui_timeout >= backend_timeout + tick_interval`).
  - didn't make configurable, left as constants with a clear relation
3. Longer term, make the commands idempotent or give the backend a reply cache keyed by
   `RequestSequence`, so a retry of an already-executed sequence returns the original
   response rather than re-executing. See [14](14-fully-specified-add-commands.md) for a
   proposal on the command side, and also [04](04-stdin-thread-dies-on-bad-line.md)
   and [05](05-waiting-channels-leak.md) — same protocol layer.

## Related

The event loop's priority ordering also means a long `fill_buckets` (which shells out to
`beet` and can take seconds) blocks command handling entirely. Worth considering whether
the slow work belongs off the event-loop thread.


## Resolution

Extracted `beet_pusher` main logic to library module to document relationship
between constants.

Split "Related" section above to [15](../15-query-beet-outside-event-loop.md)
