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
codesign --verify "$app" && echo "Signed (ad-hoc): $app"
lipo -info "$app/Contents/MacOS/silvery"

# Zip it for sharing (ditto keeps the app's signature and permissions intact)
zip="dist/Silvery-macos-$label.zip"
ditto -c -k --keepParent "$app" "$zip"
ls -la "$zip"
echo "Done. Double-click dist/Silvery.app, or send $zip to another Mac."
