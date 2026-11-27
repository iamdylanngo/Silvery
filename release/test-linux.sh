#!/bin/bash
# Tries a Linux release download on Debian 12 in Docker:
#   ./release/test-linux.sh arm64      (or x86_64)
# It unpacks dist/release/Silvery-Linux-<arch>.tar.gz, shows the newest system library
# version it needs, transcribes a spoken test clip and saves a picture of the window to shots/.
source "$(dirname "$0")/common.sh"
arch="${1:-x86_64}"
docker_arch=$([ "$arch" = arm64 ] && echo arm64 || echo amd64)
tarball="dist/release/Silvery-Linux-$arch.tar.gz"
[ -f "$tarball" ] || { echo "Make it first: ./release/linux-$arch.sh"; exit 1; }

MODELS="$HOME/Library/Application Support/Silvery/models"
[ -d "$MODELS" ] || MODELS="$HOME/.local/share/Silvery/models"
mkdir -p shots "$MODELS"
image="silvery-linux-builder-$docker_arch"
docker build -q --build-arg PLATFORM="linux/$docker_arch" -t "$image" -f docker/Dockerfile.linux docker >/dev/null
docker run --rm --platform "linux/$docker_arch" \
  -v "$ROOT":/src \
  -v "$MODELS":/root/.local/share/Silvery/models:ro \
  "$image" bash -c "
    set -e
    mkdir -p /tmp/t /root/.local/share/Silvery && tar -C /tmp/t -xzf /src/$tarball
    exe=\$(ls /tmp/t/*/silvery)
    echo \"--- needs system library (glibc) up to: \$(objdump -T \$exe | grep -o 'GLIBC_[0-9.]*' | sort -V | tail -1)\"
    echo \"--- this test system has: \$(ldd --version | head -1)\"
    if [ -f /src/shots/speech.wav ]; then
      echo '--- speech to text'; \$exe --self-test /src/shots/speech.wav 2>&1 | tail -3
    fi
    echo '--- window'
    xvfb-run -a -s '-screen 0 1400x900x24' \$exe --screenshot /src/shots/release-linux-$arch.png 2>&1 | tail -3
    ls -la /src/shots/release-linux-$arch.png"
