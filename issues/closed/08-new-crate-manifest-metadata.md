# 08 - New crates are missing `license`, `authors`, and `publish`

Status: closed
Severity: low
Introduced: `feature/beet-pusher-web`

## Problem

Five crates added on this branch omit metadata that the pre-existing crates carry:

| Crate | `license` | `authors` | `publish = false` |
|---|---|---|---|
| `beet-pusher-webui` | missing | missing | missing |
| `fake-beet` | missing | missing | missing |
| `fake-vlc` | missing | missing | missing |
| `stdio-test` | missing | missing | missing |
| `vlc-http-test` | missing | missing | missing |

Compare `crates/beet-pusher/Cargo.toml`, which sets both
`license = "GPL-3.0-or-later"` and `authors`.

## Why it matters

Every source file on the branch carries the GPL copyright header, and `checks.sh`
verifies the `COPYING` hash, so the licensing intent is clear — the manifests just don't
state it. For a GPL project, package metadata that says nothing about the licence is a
real (if small) gap, and it is far easier to fix now than after any of these are
distributed.

Separately, the four test-double crates (`fake-beet`, `fake-vlc`, `stdio-test`,
`vlc-http-test`) are internal harness code that should never reach crates.io.

## Fix

Add `license` and `authors` to all five. Consider hoisting both to
`[workspace.package]` with `license.workspace = true` in each member, so new crates
inherit them by default rather than by remembering.

Add `publish = false` to the four test-double crates.

Worth checking whether `cargo xtask checks` can assert this, given it already enforces
copyright headers per-file.

## Resolution

Added authors, license, and publish fields to all crates.
- Authors and license reference the workspace value
- Publish is `false` for internal crates, and workspace value (currently also `false`) for all others

Verified via nushell script:

```nushell
cargo metadata | from json | get packages | where $in.source == null | select name authors license publish
```
