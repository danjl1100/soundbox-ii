#!/usr/bin/env bash

rm -f example-1-table.html example-2-svg-box.html example-3-svg.html example-4-json.txt

if [ "$1" = "clean" ]; then
  echo "Cleaned."
  exit 0
fi

cargo run --example simple-html -- --render-mode table > example-1-table.html
cargo run --example simple-html -- --render-mode svg-box > example-2-svg-box.html
cargo run --example simple-html -- --render-mode svg > example-3-svg.html
cargo run --example simple-html -- --render-mode json > example-4-json.txt
