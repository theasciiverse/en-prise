#!/bin/sh
# Records asciiverse/en-prise.cast, the catalog preview (100x30), with hub's
# castrec (../../hub). Run from the repo after cargo build --release. The
# computer plays itself (--watch, Normal) with the fights in full; 10 frames
# a second keeps the file small (a fight repaints the whole view).
set -eu
hub=${HUB:-../../hub}
seed=${SEED:-7}
home=$(mktemp -d)
CAST_COLS=100 CAST_ROWS=30 HOME="$home" XDG_DATA_HOME="$home/data" \
  python3 "$hub/tools/castrec.py" asciiverse/en-prise.cast . "w45;kq" -- target/release/en-prise --watch --seed "$seed" --fps 10
rm -rf "$home"
