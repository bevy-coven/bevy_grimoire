#!/usr/bin/env bash
# Builds Cauldron.app (bevy_cauldron with every family crate), signs it, wraps it in a DMG, optionally
# notarizes and staples both, and writes the update zip and SHA256SUMS. Run from the repo root on macOS.
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

rm -rf dist
APP=dist/Cauldron.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/example"
cp "$T/release/bevy_cauldron" "$APP/Contents/MacOS/bevy_cauldron"
sed "s/@VERSION@/$V/g" "$MACOS_DIR/Info.plist.in" > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
cp "$MACOS_DIR/Cauldron.icns" "$APP/Contents/Resources/Cauldron.icns"
# The example project: tracked files only, copied to ~/Documents on first open (launcher::example_to_open).
git archive HEAD assets | tar -x -C "$APP/Contents/Resources/example"

if [ "$IDENTITY" = - ]; then
  codesign --force --options runtime --entitlements "$MACOS_DIR/entitlements.plist" --sign - "$APP"
else
  codesign --force --options runtime --timestamp --entitlements "$MACOS_DIR/entitlements.plist" --sign "$IDENTITY" "$APP"
fi
codesign --verify --deep --strict --verbose=2 "$APP"

STAGE=dist/dmg-staging
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname Cauldron -srcfolder "$STAGE" -ov -format UDZO "dist/$NAME.dmg"
rm -rf "$STAGE"
if [ "$IDENTITY" != - ]; then
  codesign --timestamp --sign "$IDENTITY" "dist/$NAME.dmg"
fi

if [ "$NOTARIZE" = 1 ]; then
  RESULT=$(xcrun notarytool submit "dist/$NAME.dmg" --key "$APPLE_NOTARY_KEY_PATH" --key-id "$APPLE_NOTARY_KEY_ID" \
    --issuer "$APPLE_NOTARY_ISSUER_ID" --wait --output-format json)
  echo "$RESULT"
  read -r STATUS ID < <(python3 -c 'import json, sys; r = json.loads(sys.argv[1]); print(r.get("status", "?"), r.get("id", "?"))' "$RESULT")
  if [ "$STATUS" != Accepted ]; then
    echo "build_app.sh: notarization $STATUS; the log:" >&2
    xcrun notarytool log "$ID" --key "$APPLE_NOTARY_KEY_PATH" --key-id "$APPLE_NOTARY_KEY_ID" --issuer "$APPLE_NOTARY_ISSUER_ID" >&2 || true
    exit 1
  fi
  # One submission covers both: the ticket is looked up by the app's cdhash.
  xcrun stapler staple "dist/$NAME.dmg"
  xcrun stapler staple "$APP"
fi

# After stapling, so an update carries the ticket. `ditto` keeps the signature intact; `zip` does not.
ditto -c -k --keepParent "$APP" "dist/$NAME.zip"
(cd dist && shasum -a 256 "$NAME.dmg" "$NAME.zip" > SHA256SUMS)
cat dist/SHA256SUMS
