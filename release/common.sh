#!/bin/bash
# Shared by the macOS and Linux release scripts. Not meant to be run on its own.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)

# Downloads must run on other people's computers, so don't tune the speech engine to this
# computer's processor, and keep these builds in their own folder.
export GGML_NATIVE=OFF
export CARGO_TARGET_DIR="$ROOT/target/release-portable"
OUT="$ROOT/dist/release"
mkdir -p "$OUT"

say() { echo "==> $*"; }

# build_linux <x86_64|arm64>: builds the Linux program and prints its path.
# On a Linux computer with the same kind of processor it builds directly; anywhere else
# (a Mac, or the other processor) it builds inside Docker on Debian 12.
build_linux() {
  local arch="$1" docker_arch host_arch
  docker_arch=$([ "$arch" = arm64 ] && echo arm64 || echo amd64)
  host_arch=$(uname -m); [ "$host_arch" = aarch64 ] && host_arch=arm64

  if [ "$(uname -s)" = Linux ] && [ "$host_arch" = "$arch" ]; then
    say "Building on this Linux computer ($arch)" >&2
    cargo build --release >&2 || { echo "The build failed." >&2; exit 1; }
    echo "$CARGO_TARGET_DIR/release/silvery"
    return
  fi

  command -v docker >/dev/null || { echo "Docker is needed to build the Linux $arch version here." >&2; exit 1; }
  say "Building in Docker (Debian 12, $arch)" >&2
  local image="silvery-linux-builder-$docker_arch"
  docker build -q --build-arg PLATFORM="linux/$docker_arch" -t "$image" -f docker/Dockerfile.linux docker >/dev/null \
    || { echo "Couldn't prepare the Linux build machine (see the message above)." >&2; exit 1; }
  docker run --rm --platform "linux/$docker_arch" \
    -v "$ROOT":/src \
    -v silvery-cargo-registry:/usr/local/cargo/registry \
    -v "silvery-release-target-$docker_arch":/target \
    -e CARGO_TARGET_DIR=/target \
    -e GGML_NATIVE=OFF \
    "$image" bash -c "cargo build --release && mkdir -p /src/target/linux-$arch && cp /target/release/silvery /src/target/linux-$arch/silvery" >&2 \
    || { echo "The Linux $arch build failed." >&2; exit 1; }
  echo "$ROOT/target/linux-$arch/silvery"
}

# package_linux <arch> <program>: makes dist/release/Silvery-<version>-Linux-<arch>.tar.gz with the
# program, its icon, an applications-menu entry and install.sh.
package_linux() {
  local arch="$1" exe="$2"
  local name="Silvery-$VERSION-Linux-$arch"
  local tmp; tmp=$(mktemp -d)
  local d="$tmp/$name"
  mkdir -p "$d"
  cp "$exe" "$d/silvery"; chmod 755 "$d/silvery"
  cp assets/logo-256.png "$d/silvery.png"
  cp LICENSE "$d/LICENSE"
  cat > "$d/silvery.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Silvery
Comment=Private meeting notes with a local AI
Exec=silvery
Icon=silvery
Categories=Office;AudioVideo;
Terminal=false
EOF
  cat > "$d/install.sh" <<'EOF'
#!/bin/sh
# Installs Silvery for this user: the program in ~/.local/bin, plus a menu entry and icon.
set -e
cd "$(dirname "$0")"
mkdir -p ~/.local/bin ~/.local/share/applications ~/.local/share/icons/hicolor/256x256/apps
install -m 755 silvery ~/.local/bin/silvery
install -m 644 silvery.png ~/.local/share/icons/hicolor/256x256/apps/silvery.png
sed "s|^Exec=.*|Exec=$HOME/.local/bin/silvery|" silvery.desktop > ~/.local/share/applications/silvery.desktop
echo "Silvery is installed. Find it in your applications menu, or run: ~/.local/bin/silvery"
EOF
  chmod 755 "$d/install.sh"
  # Leave out Mac-only file details, which Linux's tar warns about.
  if tar --version 2>/dev/null | grep -q bsdtar; then
    COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs -C "$tmp" -czf "$OUT/$name.tar.gz" "$name"
  else
    tar -C "$tmp" -czf "$OUT/$name.tar.gz" "$name"
  fi
  rm -rf "$tmp"
  say "Made $OUT/$name.tar.gz"
}

# package_macos <intel|arm64> <file label>: builds Silvery.app and zips it into dist/release/.
package_macos() {
  local which="$1" label="$2" made
  if [ "$which" = arm64 ]; then
    # Features every Apple chip (M1 and later) has; the heavy work runs on the graphics chip anyway.
    export GGML_CPU_ARM_ARCH="armv8.2-a+dotprod+fp16"
  fi
  "$ROOT/build-macos.sh" "$which"
  made=$(ls -t dist/Silvery-macos-*.zip | head -1)
  mv "$made" "$OUT/Silvery-$VERSION-macOS-$label.zip"
  say "Made $OUT/Silvery-$VERSION-macOS-$label.zip"
}
