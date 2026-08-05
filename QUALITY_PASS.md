# Quality pass — pre-merge actions

**Temporary document.** Tracks review findings that must be resolved before
`feature/beet-pusher-web` merges into `soundbox-iii`. Delete this file as part of the merge.

- **Scope of this pass:** the five issues closed since `de7b3e81` — `closed/01`, `02`, `06`,
  `08`, `09` (commits `5ce63a6`, `24d03d2`, `eb8d714`, `5c1f72c`, `a75cdfa`).
- **Baseline:** `./checks.sh` passes at `24d03d2` (exit 0; only the pre-existing
  `proc-macro-error2` future-incompat warning).
- **Verdict:** 01 and 06 complete. 08 and 09 complete with one item each. 02 is the one that
  matters — the timeout mismatch is fixed, but the defect it was filed for (a retry after a
  caller-side timeout creating a duplicate node) is *narrowed, not closed*.

## How to use this document

Work the **Actions** table. Each row has a stable ID, so a commit message or a question can
say "QP-4" without ambiguity. Tick the box when the change lands; leave the row in place.
Each action links to its evidence in [Findings](#findings) below — read that before acting,
it has the reasoning and the `path:line` references.

When the next review batch arrives, replace the Actions table and Findings section wholesale
and update the Scope line. Anything that has outgrown this file belongs in `issues/`, not
here.

Status values: `todo` · `done` · `accepted` (reviewed, deliberately not changing).

---

## Actions

### Must land before merge

| ID | Action | Where | Status |
|---|---|---|---|
| QP-1 | Land [15](issues/15-query-beet-outside-event-loop.md) — slow `beet`/VLC work off the command loop | see issue | [ ] todo |
| QP-2 | Fix the invariant comment: it names `FILL_PLAYLIST_INTERVAL`, the code adds `TICK_INTERVAL` | `crates/beet-pusher/src/command_loop.rs:20` | [x] done |
| QP-3 | Set `Status: closed` in all five closed issue files | `issues/closed/*.md` | [x] done |

### Should land before merge

| ID | Action | Where | Status |
|---|---|---|---|
| QP-4 | Make the client budget one deadline, not the same `timeout` spent twice | `crates/beet-pusher-webui/src/infra/stdio_pipe.rs:165-173` | [x] done |
| QP-5 | Add a unit test asserting `get_client_wait_timeout() >= RESPONSE_TIMEOUT + TICK_INTERVAL` | `crates/beet-pusher/src/command_loop.rs` | [x] done |
| QP-6 | Explicit `publish = false` on `stdio-test` and `vlc-http-test` | their `Cargo.toml` | [x] done |
| QP-7 | Reword the `DEFAULT_SCRIPT` item in 09 — it is documented and retained, not removed | `issues/closed/09-...md:16` | [x] done |

### Deferred — file as issues rather than blocking the merge

| ID | Action | Status |
|---|---|---|
| QP-8 | Idempotent commands or a reply cache keyed by `RequestSequence` — the only thing that *closes* the duplicate-node race. Tracked by [14](issues/14-fully-specified-add-commands.md); QP-1 shrinks the window, this shuts it. | [ ] todo |
| QP-9 | Teach `cargo xtask checks` to assert manifest metadata (`license`/`authors`/`publish`), so the next new crate cannot land without it | [x] done |
| QP-10 | Confirm the bound port is logged at startup, now that the default port is `0` | [ ] todo |

### Accepted — no action

| ID | Item | Rationale |
|---|---|---|
| QP-11 | Cross-references inside `issues/closed/02` point at siblings (`[14](14-...)`) that now live one directory up | Rewriting links on every close is too much manual overhead. `issues/README.md` names the `closed/` directory instead, so moved files stay findable. |
| QP-12 | `.prefix_separator` is not set explicitly on the webui env config | Falls back to `separator`, which is what the callers already use. Correct today; only bites on an upstream default change. |
| QP-13 | `times_out` test still asserts the timeout path | `PipeRunner` queues no reply, so the timeout is the genuinely correct outcome. Costs ~0.7–1.4 s of wall clock, which feeds [10](issues/10-test-timing-flakiness.md). |

---

## Findings

### 02 — pipe timeout mismatch → the important one

**What was fixed, and it is good work.** The webui no longer carries its own constant:
`node_service.rs:31` calls `beet_pusher::command_loop::get_client_wait_timeout()`, so the
backend owns the number and the relationship lives in code instead of in two crates' heads
(`command_loop.rs:16-25`). That was only possible because the event loop was extracted from
`bin/beet-pusher.rs` into `src/command_loop.rs` (`313a5e0`, `86627e7`) — the right order of
operations.

**QP-2 — the comment names the wrong constant.** `command_loop.rs:20`:

```rust
// NOTE: must be larger than `RESPONSE_TIMEOUT + FILL_PLAYLIST_INTERVAL`,
// so add a suitable interval
RESPONSE_TIMEOUT + TICK_INTERVAL + ADDED_INTERVAL
```

The comment says `FILL_PLAYLIST_INTERVAL` (5 s); the code adds `TICK_INTERVAL` (100 ms). The
code matches the invariant issue 02 specified (`webui_timeout >= backend_timeout +
tick_interval`), so the value is right and the comment is wrong — but this is the one comment
whose entire job is to stop the invariant drifting. QP-5 pins it in a test.

**QP-4 — the invariant is enforced per-phase, not end-to-end.**
`stdio_pipe.rs:165-173` spends the same `timeout` twice: once on `cmd_tx.send_timeout(..)`
to enqueue, then again on `tokio::time::timeout(timeout, rx)` awaiting the reply. Worst case
the webui waits ~1.4 s, not the 700 ms the constant implies. This errs in the safe direction
(caller waits longer than callee), so it does not reintroduce the bug — but "the webui's
budget" is not a single number, and future reasoning about the invariant will assume it is.
A single shared `Instant` deadline fixes it.

**QP-1 / QP-8 — the headline defect is narrowed, not eliminated.** Issue 02's goal for step 1
was "a timeout on the caller side implies the callee has also given up." That now holds for
the webui↔pipe-thread hop, but **not** for the pipe-thread↔event-loop hop.
`command_loop.rs:348-356` pushes the event into the channel, waits `RESPONSE_TIMEOUT`
(500 ms), and on expiry returns `ErrorKind::InternalTimeout` — **leaving the event queued for
the loop to execute anyway**. So the original failure mode survives whenever the loop is busy
past 500 ms:

- `LoopEvent::MaintainPlaylist` does blocking VLC HTTP (`command_loop.rs:113`), and
- `LoopEvent::MaintainBuckets` shells out to `beet` per bucket (`beet.rs:47-57` →
  `beet/query.rs:85` `command.output()`), seconds at a time,

and both sit *ahead* of `loop_rx.recv_timeout` in the priority ladder
(`command_loop.rs:95-107`). The webui reports `fail`, the backend then creates the bucket,
the client retries, two buckets exist — exactly the scenario 02 was filed for.

This is not a regression. The fix removed the *guaranteed* mismatch (100 ms vs 500 ms, which
failed even on an idle loop) and left the *contended* one. Closing 02 is defensible only
because the remainder is tracked: QP-1 shrinks the window, QP-8 closes it. Neither is
optional if the duplicate-create is to actually go away.

Related, same root cause: `loop_tx.as_ref().send(event)` at `command_loop.rs:348` is a
`sync_channel(1)` send with no timeout at all, so a second concurrent command can block the
pipe thread indefinitely before its own 500 ms budget starts. Covered by issue 15.

### 08 — new crate manifest metadata → complete, one gap

All 17 workspace crates carry `authors.workspace = true` and `license.workspace = true`; the
root sets `authors`, `license = "GPL-3.0-or-later"`, `publish = false`. Verified via
`cargo metadata`.

**QP-6.** Issue 08 asked for explicit `publish = false` on the four test-double crates. Only
`fake-beet` and `fake-vlc` got it (plus `xtask`); `stdio-test` and `vlc-http-test` inherit
`publish.workspace = true`, which resolves to `false` only because the workspace default is
`false`. Identical effect today — but the day the workspace flips to publishable, two
internal harness crates silently become publishable with it. One line each.

**QP-9.** Issue 08 also raised, as "worth checking", whether `cargo xtask checks` could
assert this. It does not, so the next new crate can still land without metadata; verification
was a manual `cargo metadata` one-liner recorded in the resolution.

### 09 — commented-out code and lint opt-outs → complete, one deviation

Verified removed: `SpigotEmptyError`, `AppError::NotFound`, `fake-vlc`'s
`lock_set_current_playing`, the `stdio_pipe` `trace!`, `base_beet_config`, and the
`setup_spigot` empty-spigot bail. All blanket opt-outs are gone — no `#![expect(missing_docs)]`
remains in `beet-pusher-webui/src/lib.rs`, `tests/entrypoint.rs`, or
`beet-pusher/src/pipe_exec.rs`, and every remaining `missing_docs` expect in the workspace is
a narrow per-item one with a concrete reason. The `too_many_lines` expects on `beet-pusher.rs`
`main` and `xtask` `WebUiSpawn::spawn` are gone, both functions having been split (`6827535`,
`313a5e0`, `d97b53f`).

**QP-7.** The `DEFAULT_SCRIPT` item is checked `[x]`, but the commented-out sample script is
still at `crates/beet-pusher/src/bin/beet-pusher.rs:33-46`. What changed is that a `NOTE` now
explains it is a deliberate testing aid pending persisted spigot state. That answers the
issue's real concern ("a meaningful behavioural choice disguised as commented-out example
content") and is arguably better than deleting a genuinely useful sample — the checkbox just
overstates it. This is the only place a `[x]` does not match the tree.

### 06 — webui config reads the unprefixed environment → complete

`config.rs` uses `Environment::default().prefix("BEET_PUSHER_WEBUI").separator("__")` with
defaults for both required fields, and both callers were updated to the prefixed names
(`crates/xtask/src/beet_pusher.rs:111-112`,
`crates/beet-pusher-webui/tests/common/end_to_end/pipe_runner.rs:22-24`). The optional asks
were done too: `SCRIPT_WRITE_PORT` is now a `Config` field rather than a raw
`std::env::var`, and `LOG_JSON` is documented as intentionally unscoped (`src/lib.rs:27`).

**QP-10.** The default port is `0` (ephemeral) rather than the fixed port issue 06 suggested.
Defensible — it can never collide — but it makes the service undiscoverable unless
`script_write_port` is set or the bound port is logged. Worth confirming the latter before
anyone runs it outside `xtask`.

### 01 — `fake-beet` should be a dev-dependency → complete

`crates/beet-pusher/Cargo.toml` lists it under `[dev-dependencies]` (with `fake-vlc`,
`stdio-test`, `vlc-http-test`, `vlc-http-auth`); nothing under `src/` referenced it and the
build passes. The secondary ask holds: `xtask` still depends on `fake-beet`/`fake-vlc`
normally, which is correct — it drives them at runtime.

No follow-up.
