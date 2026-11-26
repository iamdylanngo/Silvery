#!/bin/bash
# Builds Silvery for macOS as a real app (Silvery.app) plus a zip file for sharing. Results go to dist/:
#   ./build-macos.sh            for this Mac's chip (Apple chip M1/M2/M3/M4… or Intel)
#   ./build-macos.sh arm64      Apple-chip Macs only
#   ./build-macos.sh intel      Intel Macs only
#   ./build-macos.sh universal  one app that runs on both Apple-chip and Intel Macs
# Needs: Rust (https://rustup.rs), Xcode Command Line Tools (xcode-select --install), CMake (brew install cmake).
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

what="${1:-native}"
if [ "$what" = native ]; then
  [ "$(uname -m)" = arm64 ] && what=arm64 || what=intel
fi

build_for() { # <rust target> → prints the path of the built program
  rustup target add "$1" >/dev/null 2>&1 || true
  # Intel Macs from macOS 11 on, Apple-chip Macs from macOS 11 on
  MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release --target "$1" >&2
  echo "${CARGO_TARGET_DIR:-target}/$1/release/silvery"
}

case "$what" in
  arm64)     exe=$(build_for aarch64-apple-darwin); label=apple-chip ;;
  intel)     exe=$(build_for x86_64-apple-darwin);  label=intel ;;
  universal)
    a=$(build_for aarch64-apple-darwin)
    b=$(build_for x86_64-apple-darwin)
    u="${CARGO_TARGET_DIR:-target}/universal"
    mkdir -p "$u"
    lipo -create -output "$u/silvery" "$a" "$b"
    exe="$u/silvery"; label=universal ;;
  *) echo "Unknown choice: $what (use arm64, intel or universal)"; exit 1 ;;
esac

# Make dist/Silvery.app: the program, its Info.plist (with the microphone description macOS needs)
# and an ad-hoc signature. The program knows how to package itself (--install-app).
mkdir -p dist
SILVERY_APP_DIR="$PWD/dist" "$exe" --install-app
app="dist/Silvery.app"
zip="dist/Silvery-macos-$label.zip"

# Optional: sign with an Apple Developer ID and notarize, so macOS opens Silvery on other Macs
# without the "Apple could not verify…" warning. Set these to switch it on (see docs/releasing.md):
#   SILVERY_SIGN_IDENTITY   e.g. "Developer ID Application: Your Name (TEAMID)"
#   SILVERY_NOTARY_APPLE_ID, SILVERY_NOTARY_TEAM_ID, SILVERY_NOTARY_PASSWORD (an app-specific password)
if [ -n "${SILVERY_SIGN_IDENTITY:-}" ]; then
  echo "==> Signing with: $SILVERY_SIGN_IDENTITY"
  codesign --force --deep --options runtime --timestamp \
    --entitlements assets/Silvery.entitlements \
    --sign "$SILVERY_SIGN_IDENTITY" "$app"
  codesign --verify --strict --verbose=1 "$app"
  if [ -n "${SILVERY_NOTARY_APPLE_ID:-}" ] && [ -n "${SILVERY_NOTARY_PASSWORD:-}" ] && [ -n "${SILVERY_NOTARY_TEAM_ID:-}" ]; then
    echo "==> Sending to Apple for notarization (usually a few minutes)…"
    ditto -c -k --keepParent "$app" "$zip"
    xcrun notarytool submit "$zip" \
      --apple-id "$SILVERY_NOTARY_APPLE_ID" --team-id "$SILVERY_NOTARY_TEAM_ID" \
      --password "$SILVERY_NOTARY_PASSWORD" --wait
    xcrun stapler staple "$app"
    spctl --assess --type execute --verbose "$app" && echo "==> Notarized: macOS will open it without warnings."
  else
    echo "==> Signed but not notarized (set the SILVERY_NOTARY_* values to notarize)."
  fi
else
  codesign --verify "$app" && echo "Signed (ad-hoc): $app"
  echo "    Not notarized: on other Macs, open it the first time via System Settings › Privacy & Security › Open Anyway."
fi
lipo -info "$app/Contents/MacOS/silvery"

# Zip it for sharing (ditto keeps the app's signature and permissions intact)
ditto -c -k --keepParent "$app" "$zip"
ls -la "$zip"
echo "Done. Double-click dist/Silvery.app, or send $zip to another Mac."
