#!/usr/bin/env bash
#
# Helper to launch the requesite tools for developing the `frontend` crate
#  1) ./dev_shell.sh vlc          (launches VLC)
#  2) ./dev_shell.sh              (launches soundbox-ii server)
#  3) cd frontend && ./watch.sh   (launches trunk build watcher)
#
export VLC_HOST=127.0.0.1
export VLC_BIND_HOST=${VLC_HOST}
export VLC_PORT=8891
export VLC_PASSWORD=notsecure_at_all

APP=${1:-soundbox-ii_bin}
shift
if [ $# -gt 0 ]; then
  ARGS=$*
else
  ARGS="--serve --watch-assets --interactive"
fi

if [ "$IN_NIX_SHELL" != "" ]; then
  which cargo >/dev/null 2>&1
  if [ $? -eq 0 ]; then
    echo 'Executing cargo directly (within nix shell)'
    echo ''
    cargo run -- ${ARGS}
    exit $?
  fi
  echo 'No cargo found in this nix shell (fall back to `nix run`)'
else
  echo 'No nix shell detected (run `nix develop` to speed this up for development)'
fi

echo ''
nix run .#${APP} -- ${ARGS}
