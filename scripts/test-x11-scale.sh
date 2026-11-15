#!/usr/bin/env bash
# Unit tests for the XSETTINGS scale factor of the vendored GPUI X11 backend
# (docs/decisions/x11-desktop-scale.md).
#
# The crate only compiles on Linux with its system libraries, and the module
# is std-only, so it is built standalone with rustc --test; this runs on any
# host.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODULE="$ROOT/third_party/gpui-pre-linux/src/linux/x11/xsettings_scale.rs"

OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT
rustc --edition 2024 --test --crate-name xsettings_scale -o "$OUT/xsettings_scale" "$MODULE"
"$OUT/xsettings_scale"
