#!/usr/bin/env bash
# Builds Cauldron.app (bevy_cauldron with every family crate, and its MCP server bevy_cauldron_mcp beside it), signs it,
# wraps it in a DMG, optionally notarizes and staples both, and writes the update zip and SHA256SUMS. Run from the repo
# root on macOS.
#
#   CAULDRON_SIGN_IDENTITY  codesign identity; default `-` (ad-hoc, no timestamp, no notarization)
#   CAULDRON_NOTARIZE       1 to notarize (needs APPLE_NOTARY_KEY_PATH, APPLE_NOTARY_KEY_ID, APPLE_NOTARY_ISSUER_ID)
#
# Output in dist/: Cauldron.app, Cauldron-<V>-macos-arm64.dmg, Cauldron-<V>-macos-arm64.zip, SHA256SUMS.
# The asset names are what the in-app updater looks for (crates/bevy_cauldron/src/update.rs, zip_name).
set -euo pipefail

IDENTITY="${CAULDRON_SIGN_IDENTITY:--}"
NOTARIZE="${CAULDRON_NOTARIZE:-0}"
MACOS_DIR=crates/bevy_cauldron/macos

[ "$(uname -s)" = Darwin ] || { echo "build_app.sh: macOS only" >&2; exit 1; }
[ -f "$MACOS_DIR/Info.plist.in" ] || { echo "build_app.sh: run from the repository root" >&2; exit 1; }
if [ "$NOTARIZE" = 1 ]; then
  : "${APPLE_NOTARY_KEY_PATH:?CAULDRON_NOTARIZE=1 needs APPLE_NOTARY_KEY_PATH}"
  : "${APPLE_NOTARY_KEY_ID:?CAULDRON_NOTARIZE=1 needs APPLE_NOTARY_KEY_ID}"
  : "${APPLE_NOTARY_ISSUER_ID:?CAULDRON_NOTARIZE=1 needs APPLE_NOTARY_ISSUER_ID}"
  [ "$IDENTITY" != - ] || { echo "build_app.sh: an ad-hoc signature cannot be notarized" >&2; exit 1; }
fi

# The version and the target directory from cargo itself: never a hard-coded target/.
read -r V T < <(cargo metadata --format-version 1 --no-deps | python3 -c '
import json, sys
m = json.load(sys.stdin)
print(next(p["version"] for p in m["packages"] if p["name"] == "bevy_cauldron"), m["target_directory"])')
NAME="Cauldron-$V-macos-arm64"
echo "build_app.sh: Cauldron $V, identity $IDENTITY, notarize $NOTARIZE"

cargo build --release -p bevy_cauldron --features family_all
# The editor's own MCP server ships inside the app (#306), so an agent can drive an installed copy:
# `ssh <mac> /Applications/Cauldron.app/Contents/MacOS/bevy_cauldron_mcp`.
cargo build --release -p bevy_cauldron_mcp

rm -rf dist
APP=dist/Cauldron.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/example"
cp "$T/release/bevy_cauldron" "$APP/Contents/MacOS/bevy_cauldron"
cp "$T/release/bevy_cauldron_mcp" "$APP/Contents/MacOS/bevy_cauldron_mcp"
sed "s/@VERSION@/$V/g" "$MACOS_DIR/Info.plist.in" > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
cp "$MACOS_DIR/Cauldron.icns" "$APP/Contents/Resources/Cauldron.icns"
# The example project: tracked files only, copied to ~/Documents on first open (launcher::example_to_open).
git archive HEAD assets | tar -x -C "$APP/Contents/Resources/example"
# The What's new page's stills, where launcher::still_image looks: Contents/Resources/<the path the changelog gives>.
# Only the entries the page lists, so the app carries a few images and not every clip's still. The stills are Git LFS
# objects: release-macos.yml pulls them, and a pointer file that is still one is left out (the page lists that entry without
# its picture) with a warning.
NEWS_SHOWN=10 # launcher::NEWS_SHOWN; a test in launcher.rs fails when the two differ
while read -r still; do
  [ -n "$still" ] || continue
  if head -c 40 "$still" | grep -q '^version https://git-lfs'; then
    echo "build_app.sh: $still is an LFS pointer, not copied (git lfs pull --include=\"docs/changelog/media/*.png\")" >&2
    continue
  fi
  mkdir -p "$APP/Contents/Resources/$(dirname "$still")"
  cp "$still" "$APP/Contents/Resources/$still"
done < <(python3 tools/release/macos/news_stills.py CHANGELOG.md "$NEWS_SHOWN")

# The nested binary is signed first, with the app's own flags: `codesign` on the bundle signs only its main executable,
# and notarization refuses a bundle with an unsigned (or untimestamped) executable inside it.
if [ "$IDENTITY" = - ]; then
  codesign --force --options runtime --sign - "$APP/Contents/MacOS/bevy_cauldron_mcp"
  codesign --force --options runtime --entitlements "$MACOS_DIR/entitlements.plist" --sign - "$APP"
else
  codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP/Contents/MacOS/bevy_cauldron_mcp"
  codesign --force --options runtime --timestamp --entitlements "$MACOS_DIR/entitlements.plist" --sign "$IDENTITY" "$APP"
fi
codesign --verify --deep --strict --verbose=2 "$APP"

# **The app is notarized and stapled before it goes into the DMG** (measured 2026-11-04, release 0.2.0: submitting the
# DMG alone left the app inside it with no ticket — `stapler validate` on the mounted app said "does not have a ticket
# stapled to it" — so a first launch with no network had to ask Apple). The DMG is then built *around* the stapled app
# and submitted too: its own ticket is what Gatekeeper reads when the download is opened, and Apple's service answers
# an already-notarized payload fast. Both submissions go through `notarize`, so there is one wording for a failure.
notarize() { # <file to submit> <what to staple>
  [ "$NOTARIZE" = 1 ] || return 0
  RESULT=$(xcrun notarytool submit "$1" --key "$APPLE_NOTARY_KEY_PATH" --key-id "$APPLE_NOTARY_KEY_ID" \
    --issuer "$APPLE_NOTARY_ISSUER_ID" --wait --output-format json)
  echo "$RESULT"
  read -r STATUS ID < <(python3 -c 'import json, sys; r = json.loads(sys.argv[1]); print(r.get("status", "?"), r.get("id", "?"))' "$RESULT")
  if [ "$STATUS" != Accepted ]; then
    echo "build_app.sh: notarization of $1: $STATUS; the log:" >&2
    xcrun notarytool log "$ID" --key "$APPLE_NOTARY_KEY_PATH" --key-id "$APPLE_NOTARY_KEY_ID" --issuer "$APPLE_NOTARY_ISSUER_ID" >&2 || true
    exit 1
  fi
  xcrun stapler staple "$2"
  xcrun stapler validate "$2"
}

# A zip carries the app to the service with its signature and bundle shape intact; `zip` would not.
if [ "$NOTARIZE" = 1 ]; then
  ditto -c -k --keepParent "$APP" "dist/$NAME.notarize.zip"
  notarize "dist/$NAME.notarize.zip" "$APP"
  rm -f "dist/$NAME.notarize.zip"
fi

STAGE=dist/dmg-staging
mkdir -p "$STAGE"
# `ditto`, not `cp -R`: the notarization ticket lives in the bundle's metadata, and `cp -R` is not guaranteed to carry it
# into the staging folder (the DMG is built from that copy, so a dropped ticket is a DMG whose app cannot open offline).
ditto "$APP" "$STAGE/Cauldron.app"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname Cauldron -srcfolder "$STAGE" -ov -format UDZO "dist/$NAME.dmg"
rm -rf "$STAGE"
if [ "$IDENTITY" != - ]; then
  codesign --timestamp --sign "$IDENTITY" "dist/$NAME.dmg"
fi

if [ "$NOTARIZE" = 1 ]; then
  notarize "dist/$NAME.dmg" "dist/$NAME.dmg"
fi

# After stapling, so an update carries the ticket. `ditto` keeps the signature intact; `zip` does not.
ditto -c -k --keepParent "$APP" "dist/$NAME.zip"
(cd dist && shasum -a 256 "$NAME.dmg" "$NAME.zip" > SHA256SUMS)
cat dist/SHA256SUMS
