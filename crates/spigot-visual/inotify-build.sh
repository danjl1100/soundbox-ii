#!/usr/bin/env nix-shell
#! nix-shell -i bash -p inotify-tools

# This script helps with development loop of integrating script modules.
# (similar to cargo-watch or bacon, but for tsc)
#
# NOTE: The built javascript (./static) needs to be *OUTSIDE* of the watched folders.
#       Otherwise, each rebuild will trigger another rebuild.
# NOTE: xtask uses exec() to replace itself with the server process,
#       so killing the xtask PID kills the server directly.

cd "$(dirname "$0")"

# Track the server PID
SERVER_PID=""

# Function to kill the server if it's running
kill_server() {
  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "Killing server (PID: $SERVER_PID)..."
    kill "$SERVER_PID"
    wait "$SERVER_PID" 2>/dev/null
  fi
}

# Ensure we kill the server on script exit
trap kill_server EXIT

# Start the server initially in the background
echo "Starting server..."
cargo xtask spigot-visual-run &
SERVER_PID=$!
echo "Server started (PID: $SERVER_PID)"

# Use process substitution to avoid subshell and maintain SERVER_PID scope
while read -r f; do
  echo "Skipped $(timeout 1 cat | wc -l) further event(s).";
  kill_server
  echo "Restarting server..."
  cargo xtask spigot-visual-run &
  SERVER_PID=$!
  echo "Server restarted (PID: $SERVER_PID)"
done < <(inotifywait -m -r ./static-ts ./src -e move)


