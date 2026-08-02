# 11 - Generated config files are untracked rather than ignored

Status: open
Severity: low

## Problem

Running the new tooling leaves files in the working tree that `git status` reports as
untracked:

- `beet-pusher.config.toml` — written automatically by
  `crates/beet-pusher/src/bin/beet-pusher.rs:96-107` when no config file is found (the
  binary writes a template and exits with a message).
- `crates/vlc-http/vlc.toml` — VLC auth config.
- `beet-pusher-webui-spawn` — a stray build artifact copied to the repo root.

`.gitignore` currently covers only `/target`, `/dist`, `/result`, `*/target`, `.DS_Store`.

## Why it matters

Files the tooling generates on a normal first run should never appear as untracked noise.
More importantly, `beet-pusher.config.toml` and `vlc.toml` hold local paths and VLC
credentials — a stray `git add -A` commits them.

## Fix

Add to `.gitignore`:

```
/beet-pusher.config.toml
/beet-pusher-webui-spawn
vlc.toml
```

Consider whether the default config path should be somewhere outside the repo entirely
(an XDG config directory), which would remove the problem at the source rather than
papering over it. The `BEET_PUSHER_CONFIG_FILE` env var added on this branch already
makes the location overridable.
