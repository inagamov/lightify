#!/bin/sh

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
cp "$binary" "$contents/MacOS/lightify.new"
mv "$contents/MacOS/lightify.new" "$contents/MacOS/lightify"

exec "$contents/MacOS/lightify" "$@"
