# soundbox-iii
```text
~~ Don't keep your sounds boxed up ~~
```

### Development

Project organization:
- Web frontend
  - `crates/beet-pusher-webui` - web frontend (static Svelte compiled assets) and API to interface with `beet-pusher`
  - `crates/spigot-visual` - original vanilla JS / typescript prototype of displaying the track selection to the user
- Backend services
  - `crates/beet-pusher` - monitors the VLC playlist (via `vlc-http`) and feeds queued items from the source `bucket-spigot`
- Algorithms
  - `crates/bucket-spigot` - collection, selects items from a queryable source, organized by a tree of nodes (buckets) with filters and routing/selection rules at each node
  - `crates/vlc-http` - generates HTTP requests to execute VLC commands, including filling the playlist. Sibling crates listed below:
    - `vlc-http-auth` defines VLC autentication types (and CLI usage in `vlc-http-auth-clap`)
    - `vlc-http-cmd` defines VLC command and action types (and CLI usage in `vlc-http-cmd-clap`)
  - `crates/vlc-http-ureq` converts requests and responses for use with a `ureq` backend
- Misc Helpers
  - `crates/arg-util` - generic CLI usability helpers
  - `crates/shared` - copyright notices
  - `crates/xtask` - implements workspace-wide quality checks and tasks for ease of use

### Other Notable Folders
- `npins` - nixpkgs version pinning for development tools (excluding rust toolchain)
- `supply-chain` - state files for `cargo-vet` rust supply chain validation
