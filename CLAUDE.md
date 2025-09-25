## Commands

### Quality Assurance
- **`./checks.sh`** - Run comprehensive quality checks (formatting, linting, tests, documentation)
- **`cargo xtask checks fix`** - Apply automatic fixes where possible

### Development
- **`cargo xtask spigot-visual-run`** - Compile TypeScript and run the visual web server
- **`cargo xtask spigot-visual-dist`** - Compile TypeScript bindings only

### Single Test Execution
- **`cargo test --package <crate> <test_name>`** - Run specific test in a crate

## Architecture

This is a Cargo workspace with 7 crates implementing a music playlist controller system:

### Core Architecture
- **`bucket-spigot`** - Central library implementing network-based item sequencing with a "spigot" managing paths to joints and buckets for ordering items
- **`beet-pusher`** - Music management application integrating with Beets music library
- **`vlc-http`** - VLC media player HTTP API client with CLI interface
- **`spigot-visual`** - Web visualization server with TypeScript frontend for bucket-spigot data
- **`arg-util`** - Utility library for argument handling and multi-source functionality
- **`shared`** - Common types and license information
- **`xtask`** - Build automation following cargo-xtask pattern

### Key Patterns
- **Rust-TypeScript Integration**: `bucket-spigot` generates TypeScript bindings via `ts-rs`, consumed by `spigot-visual`
- **Strict Code Quality**: Workspace-level lints forbid `unsafe`, `unwrap`, `panic`; require comprehensive documentation
- **Property-Based Testing**: Uses `arbitrary` and `arbtest` for thorough testing
- **Snapshot Testing**: Uses `insta` crate for regression testing
