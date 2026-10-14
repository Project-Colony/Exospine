#!/usr/bin/env bash
# Copies the Colony theme tokens from Project-Colony-Resources into
# assets/ui/vendor/colony/, at the commit pinned below:
#
#   colony-themes.css  generated/css/colony-themes.css, unchanged
#   themes.js          generated/themes.json, unchanged, wrapped as an ES
#                      module ("export default <json>;") so the webview can
#                      import it like the rest of the UI
#
# To take a newer Resources release, set RESOURCES_SHA to its commit and run
# this script. With --check it changes nothing and fails when the vendored
# files differ from that commit (CI runs it that way).
set -euo pipefail

RESOURCES_SHA=8931f2b38422fe75757db6218a6bcbc8caa06a85
BASE="https://raw.githubusercontent.com/Project-Colony/Project-Colony-Resources/${RESOURCES_SHA}/generated"
DEST="$(cd "$(dirname "$0")/.." && pwd)/assets/ui/vendor/colony"

tmp="$(mktemp -d)"
trap 'rm -r "$tmp"' EXIT

curl -fsSL "$BASE/css/colony-themes.css" -o "$tmp/colony-themes.css"
curl -fsSL "$BASE/themes.json" -o "$tmp/themes.json"
{
  printf 'export default '
  cat "$tmp/themes.json"
  printf ';\n'
} > "$tmp/themes.js"

if [[ "${1:-}" == "--check" ]]; then
  status=0
  for f in colony-themes.css themes.js; do
    if ! cmp -s "$tmp/$f" "$DEST/$f"; then
      echo "assets/ui/vendor/colony/$f differs from Resources $RESOURCES_SHA" >&2
      status=1
    fi
  done
  exit "$status"
fi

mkdir -p "$DEST"
cp "$tmp/colony-themes.css" "$tmp/themes.js" "$DEST/"
echo "Vendored the Colony tokens of Resources $RESOURCES_SHA"
