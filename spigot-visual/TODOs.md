- [x] fix the inotify-build.sh script, to run the server in the background and kill it when files change

- [x] change the values for the three "control-button symbol" class elements in app.ts to use hex codes instead

- [ ] deduplicate spigot-visual bindings types, just use `#[cfg(features="ts-rs")]` to change the types that need changing (with corresponding From logic changes, e.g. Path parse from string, etc)
