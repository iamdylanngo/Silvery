#!/bin/bash
# Builds Silvery for Linux inside Docker (works from macOS or Linux). Results go to dist/:
#   ./build-linux.sh         both versions below
#   ./build-linux.sh x86_64  silvery-linux-x86_64  for Intel/AMD PCs
#   ./build-linux.sh arm64   silvery-linux-arm64   for ARM: Ubuntu on Apple-chip Macs (UTM/Parallels),
#                                                  Raspberry Pi 4/5, ARM servers
#   ./build-linux.sh test [x86_64|arm64]           try a build: speech-to-text + a picture of the window
set -euo pipefail
cd "$(dirname "$0")"
source docker/run.sh
mkdir -p dist shots

what="${1:-all}"

if [ "$what" = test ]; then
  arch="${2:-x86_64}"
  docker_arch=$([ "$arch" = arm64 ] && echo arm64 || echo amd64)
  MODELS="$HOME/Library/Application Support/Silvery/models"
  [ -d "$MODELS" ] || MODELS="$HOME/.local/share/Silvery/models"
  docker build -q --build-arg PLATFORM="linux/$docker_arch" -t "silvery-builder-$docker_arch" docker >/dev/null
  docker run --rm --platform "linux/$docker_arch" \
    -v "$PWD":/src \
    -v "$MODELS":/root/.local/share/Silvery/models:ro \
    "silvery-builder-$docker_arch" bash -c "
      mkdir -p /root/.local/share/Silvery
      if [ -f /src/shots/speech.wav ]; then
        echo '--- speech to text'; /src/dist/silvery-linux-$arch --self-test /src/shots/speech.wav 2>&1 | tail -4
      fi
      echo '--- window'
      xvfb-run -a -s '-screen 0 1400x900x24' /src/dist/silvery-linux-$arch --screenshot /src/shots/linux-$arch.png 2>&1 | tail -5
      ls -la /src/shots/linux-$arch.png"
  exit
fi

if [ "$what" = all ] || [ "$what" = x86_64 ]; then
  echo "=== Linux for Intel/AMD (x86_64) ==="
  run amd64 'cargo build --release && cp /target/release/silvery /src/dist/silvery-linux-x86_64'
fi

if [ "$what" = all ] || [ "$what" = arm64 ]; then
  echo "=== Linux for ARM (arm64) ==="
  run arm64 'cargo build --release && cp /target/release/silvery /src/dist/silvery-linux-arm64'
fi

ls -la dist
