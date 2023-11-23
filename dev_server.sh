#!/usr/bin/env bash
#
# Helper to launch the requesite tools for developing the `frontend` crate
#  1) ./dev_shell.sh vlc          (launches VLC)
#  2) ./dev_shell.sh              (launches soundbox-ii server)
#  3) cd frontend && ./watch.sh   (launches trunk build watcher)
#
source .envrc

APP=${1:-soundbox-ii_bin}
shift
if [ $# -gt 0 ]; then
  ARGS=$*
else
  ARGS="--serve --WATCH_ASSETS --interactive"
fi

case $APP in
  soundbox-ii_bin | soundbox-ii)
    ATTEMPT_SHELL_USAGE=1

    # auto-detect missing beet, to use fake-beet instead
    if [ "$BEET_CMD" = "" ]; then
      which beet > /dev/null
      if [ $? -ne 0 ]; then
        echo "\"beet\" not found in path, using fake-beet..."
        export BEET_CMD="`nix build .#fake-beet --no-link --print-out-paths`/bin/fake-beet"
      fi
    fi

    ;;
  vlc | cvlc)
    uname -a | grep "Darwin" -q
    if [ $? -eq 0 ]; then
      ./launch_vlc.sh macos
      exit $?
    fi
    ATTEMPT_SHELL_USAGE=0
    ;;
  *)
    ATTEMPT_SHELL_USAGE=0
    ;;
esac

if [ $ATTEMPT_SHELL_USAGE -eq 1 ]; then
  if [ "$IN_NIX_SHELL" != "" ]; then
    which cargo >/dev/null 2>&1
    if [ $? -eq 0 ]; then

      # fixup BEET_CMD
      which "${BEET_CMD:-beet}" >/dev/null 2>&1; err=$?
      if [ $err -ne 0 ]; then
        echo "Failed to find '${BEET_CMD:-beet}' in path."
        echo "*** NOTICE: Falling back to fake-beet"
        pushd ./fake-beet
        cargo build
        popd
        echo export BEET_CMD="./fake-beet/target/debug/fake-beet"
        export BEET_CMD="./fake-beet/target/debug/fake-beet"
      fi

      echo 'Executing cargo directly (within nix shell)'
      echo ''
      cargo run -- ${ARGS}
      exit $?
    fi
    echo 'No cargo found in this nix shell (fall back to `nix run`)'
    echo ''
  else
    echo 'No nix shell detected (run `nix develop` to speed this up for development)'
    echo ''
  fi
fi

nix run .#${APP} -- ${ARGS}
