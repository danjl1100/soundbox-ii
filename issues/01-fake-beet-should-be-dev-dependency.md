# 01 - `fake-beet` is a normal dependency of `beet-pusher`

Status: open
Severity: high
Introduced: `feature/beet-pusher-web`

## Problem

`crates/beet-pusher/Cargo.toml` lists `fake-beet` under `[dependencies]`:

```toml
fake-beet = { path = "../fake-beet" }
```

Nothing under `crates/beet-pusher/src/` references it — only `crates/beet-pusher/tests/`
does (`tests/common/end_to_end.rs`).

## Why it matters

`fake-beet` depends on `escargot`, which shells out to `cargo` at runtime to build
helper binaries. That test-harness machinery is currently linked into the shipping
`beet-pusher` binary and its dependency graph, inflating build time, binary size, and
the supply-chain audit surface for no benefit.

It also inverts the intended direction: a test double should depend on nothing, and be
depended on only by test targets.

## Fix

Move the entry to `[dev-dependencies]` in `crates/beet-pusher/Cargo.toml` and confirm
`cargo build -p beet-pusher` still succeeds.

Worth a check of the same kind across the other new crates — `xtask` depends on
`fake-beet` and `fake-vlc` as normal dependencies, which *is* correct there, since
`xtask::beet_pusher` uses them at runtime to drive the simulated dev environment.
