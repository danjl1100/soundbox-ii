# 10 - Sleep- and timeout-dependent tests

Status: open
Severity: low
Introduced: `feature/beet-pusher-web`

## Problem

The new end-to-end tests pass reliably today (full `cargo test --workspace` is green),
but several depend on wall-clock timing and will be the first things to flake on a loaded
CI runner.

- `crates/beet-pusher/tests/common/end_to_end.rs:96` — a bare
  `std::thread::sleep(from_millis(800))` with no condition attached. Purely a guess at
  how long the system needs.
- `crates/beet-pusher-webui/tests/common/end_to_end.rs:33` — the `times_out` test asserts
  on the 100 ms `NodeService::TIMEOUT` elapsing. It is testing a race, and it is also
  coupled to a constant that [02](02-pipe-timeout-mismatch.md) will change.
- `crates/fake-vlc/src/lib.rs:193-197` — `Drop for FakeVlc` busy-waits on
  `Arc::weak_count` at 1 ms intervals with no deadline. Acceptable in a test double, but
  it is an unbounded spin if a weak reference is ever held longer than expected, and it
  will hang rather than fail.
- `crates/stdio-test/src/lib.rs:221` and
  `crates/beet-pusher-webui/tests/common/end_to_end/pipe_runner.rs:41` — 10 ms poll loops
  against 1 s and 5 s deadlines. These are fine: bounded, with a real failure on timeout.

This branch already fixed one instance of exactly this class of problem — commit
`74ccf6d`, "drain test pipes in threads to fix macOS stderr timing" — which suggests the
CI environment does exercise these margins.

## Fix

- Replace the bare 800 ms sleep with a condition-based wait. `FakeVlc::wait_for_play_next`
  (which uses a `Condvar` with timeout) is the right pattern and already exists.
- Once [02](02-pipe-timeout-mismatch.md) lands, rewrite `times_out` to drive the timeout
  deterministically — e.g. via an injected `BeetPusherPipe` that never replies, rather
  than by racing a real subprocess.
- Give `Drop for FakeVlc` a deadline, and panic or log loudly on expiry so a stuck test
  reports rather than hangs.
