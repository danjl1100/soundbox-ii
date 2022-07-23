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

nix run .#${APP} -- ${ARGS}
