# 05 - `WaitingChannels` accumulates dead entries and scans linearly

Status: open
Severity: medium
Introduced: `feature/beet-pusher-web`

## Problem

`crates/beet-pusher-webui/src/infra/stdio_pipe.rs:107-144` tracks in-flight requests in a
`VecDeque<Option<Queued<RequestSequence>>>`.

Two weaknesses:

1. **`cull_expired` (line 112) only pops from the front.** It stops at the first live
   entry, so a timed-out entry sitting *behind* a still-waiting one is never removed. It
   also only runs when a response line arrives (line 59), so a quiet period does no
   cleanup at all. Over a long session the deque grows without bound.
2. **`find_take_tx` (line 128) is a linear scan**, acknowledged in the source: `// NOTE:
   linear search, hopefully responses are mostly sequential?`. Combined with (1), the
   scan cost grows with the leak.

## Why it matters

Neither is visible in short-lived tests, and neither is a correctness bug today — but
this is the hot path of a daemon intended to run for days, and the failure mode
(gradually slowing, gradually growing) is the kind that gets diagnosed late.

## Fix

Replace the deque with `HashMap<RequestSequence, oneshot::Sender<ResponseResult>>`:

- lookup becomes O(1) and `take` becomes a plain `remove`;
- culling becomes `retain(|_, tx| !tx.is_closed())` over the whole map, which fixes the
  front-only limitation for free.

Ordering is not relied on anywhere — matching is by `reply_to_seq`, not position.

While in this file, note that `waiting_tx.send(...)` at line 41 discards its result: if
the stdin thread has already exited, commands are queued to nobody and can only fail by
timeout. Worth surfacing as a real error instead.

## Related

Same layer as [04](04-stdin-thread-dies-on-bad-line.md) and
[02](02-pipe-timeout-mismatch.md).
