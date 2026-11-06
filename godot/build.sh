#!/usr/bin/env bash
# Build the tw-godot GDExtension and copy the shared library into the Godot project.
#
# Usage:
#   ./godot/build.sh            # debug
#   ./godot/build.sh release    # release
#
# The library naming below follows the Linux convention used in
# addons/tw_godot/tw_godot.gdextension. For macOS/Windows, copy the artifact to
# the path declared in that file instead.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-debug}"

cd "$ROOT"

if [[ "$PROFILE" == "release" ]]; then
  cargo build -p tw-godot --release
  SRC="$ROOT/target/release/libtw_godot.so"
  DST="$ROOT/godot/addons/tw_godot/bin/libtw_godot.linux.release.x86_64.so"
else
  cargo build -p tw-godot
  SRC="$ROOT/target/debug/libtw_godot.so"
  DST="$ROOT/godot/addons/tw_godot/bin/libtw_godot.linux.debug.x86_64.so"
fi

mkdir -p "$(dirname "$DST")"
install -m 644 "$SRC" "$DST"
echo "installed: $DST"
