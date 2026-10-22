#!/bin/sh
# Cargo runner on macOS (see .cargo/config.toml). Runs lightify from inside
# lightify.app so macOS registers its bundle id; Now Playing apps such as notch
# widgets skip players without one. It still runs in this terminal.
set -eu

binary="$1"
shift

# test binaries (lightify-<hash>) run as usual
if [ "$(basename "$binary")" != lightify ]; then
    exec "$binary" "$@"
fi

contents="$(dirname "$binary")/lightify.app/Contents"
mkdir -p "$contents/MacOS"
cp "$(dirname "$0")/Info.plist" "$contents/Info.plist"
# copy then rename, so a lightify that is still running keeps its old file
cp "$binary" "$contents/MacOS/lightify.new"
mv "$contents/MacOS/lightify.new" "$contents/MacOS/lightify"

exec "$contents/MacOS/lightify" "$@"
