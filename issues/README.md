# Issues

Known problems and follow-up work, one file per issue.

## Convention

- Filename: `NN-kebab-case-summary.md`, `NN` assigned in creation order (not priority).
- Move to `closed/` when resolved, keeping the filename, and append a `## Resolution` section.
- Each file states: what is wrong, where (`path:line`), why it matters, and a suggested fix.
- `Status:` is one of `open`, `deferred` (blocked on or better done after other work), or `closed`.

## Open

Filed from the review of `feature/beet-pusher-web` before merging to `soundbox-iii`.

| # | Issue | Severity |
|---|-------|----------|
| [02](02-pipe-timeout-mismatch.md) | webui/backend pipe timeouts mismatched; non-idempotent retries | high |
| [03](03-vlc-error-terminates-daemon.md) | Any VLC HTTP error terminates `beet-pusher` | high |
| [04](04-stdin-thread-dies-on-bad-line.md) | Malformed stdout line shuts down the webui | medium |
| [05](05-waiting-channels-leak.md) | `WaitingChannels` accumulates dead entries; linear scan | medium |
| [07](07-webui-has-no-authentication.md) | No authentication on the webui control surface | medium |
| [08](08-new-crate-manifest-metadata.md) | New crates missing `license`/`authors`/`publish` | low |
| [09](09-dead-code-and-lint-opt-outs.md) | Commented-out code and accumulating lint opt-outs | low |
| [10](10-test-timing-flakiness.md) | Sleep- and timeout-dependent tests | low |
| [11](11-gitignore-generated-files.md) | Generated config files are untracked, not ignored | low |
| [12](12-dependency-surface-growth.md) | Dependency surface roughly doubled | low |
| [13](13-webui-architecture-review.md) | Revisit the ports-and-adapters layering | deferred |
| [14](14-fully-specified-add-commands.md) | `AddBucket`/`AddJoint` do not name the node they create | medium |
