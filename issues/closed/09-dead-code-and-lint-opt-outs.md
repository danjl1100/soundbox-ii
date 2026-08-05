# 09 - Commented-out code and accumulating lint opt-outs

Status: closed
Severity: low
Introduced: `feature/beet-pusher-web`

Grouped cleanup items. None are bugs; together they erode the signal the workspace lint
configuration is meant to provide.

## Commented-out code

Several blocks were commented out rather than deleted, some marked "TODO remove if
unused":

- [x] `crates/beet-pusher/src/pusher.rs:193-207` — `SpigotEmptyError` and its `FillError`
  variant.
- [x] `crates/beet-pusher/src/bin/beet-pusher.rs:36-47` — the `DEFAULT_SCRIPT` body, leaving
  the const as `""`.
  - Instead of removing the the commented-out script string, added a comment explaining why it is useful to keep for now, until state is persisted
- [x] `crates/beet-pusher/src/bin/beet-pusher.rs:371-375` — the empty-spigot bail in
  `setup_spigot` (superseded: "Empty is a valid startup state").
- [x] `crates/beet-pusher-webui/src/error.rs:13-14, 46-49` — `AppError::NotFound`.
- [x] `crates/fake-vlc/src/lib.rs:325-338` — `lock_set_current_playing`.
- [x] `crates/beet-pusher-webui/src/infra/stdio_pipe.rs:122` — a `tracing::trace!` call.
- [x] `crates/beet-pusher/tests/common/end_to_end.rs:12` — `base_beet_config`.

Git history preserves all of it. The `DEFAULT_SCRIPT` case is the most worth resolving,
since an empty default script is a meaningful behavioural choice currently disguised as
commented-out example content.

## New clippy warning

- [x] `crates/xtask/src/beet_pusher.rs:33` — `too_many_lines` (120/100) on `WebUiSpawn::spawn`.
The only new clippy warning on the branch; everything else is clean. The function does
five distinct things (build binaries, generate configs, start fake-vlc, spawn, drive an
interactive prompt) and splits naturally.

## Lint opt-outs

- [x] `crates/beet-pusher/src/bin/beet-pusher.rs:33` —
  `#[expect(clippy::too_many_lines, reason = "TODO cleanup modules in main")]`.
- [x] `crates/beet-pusher-webui/src/lib.rs:2` and `tests/entrypoint.rs:2` —
  `#![expect(missing_docs, reason = "TODO while building")]`, opting an entire new crate
  out of the workspace's `missing_docs = "deny"`.
- [x] `crates/beet-pusher/src/pipe_exec.rs:4` — same, for the whole protocol module.

The blanket `missing_docs` opt-outs are the ones to plan for: the longer a crate grows
under them, the more expensive lifting them becomes, and `pipe_exec` is precisely the
module where the wire contract most needs documenting. Consider narrowing them to the
specific items still in flux, or setting a checkpoint (e.g. "before the third endpoint
lands") to remove them.

## Resolution

Removed dead code comments, refactored some long lines (completions marked `[x]`)
