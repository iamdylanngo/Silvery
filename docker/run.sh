#!/bin/bash
# Shared helper for the build scripts: run <amd64|arm64> '<command>' in a Docker build machine
# for that kind of processor. The app folder is mounted at /src; build output stays in a Docker volume.
#
# Linux builds use docker/Dockerfile.linux (Debian 12, so the programs also run on older Linux).
# The Windows cross-build sets SILVERY_DOCKERFILE=docker/Dockerfile (newer Debian with MinGW).
run() {
  local arch="$1"
  local dockerfile="${SILVERY_DOCKERFILE:-docker/Dockerfile.linux}"
  local kind; kind=$([ "$dockerfile" = docker/Dockerfile.linux ] && echo linux || echo mingw)
  local image="silvery-$kind-builder-$arch"
  docker build -q --build-arg PLATFORM="linux/$arch" -t "$image" -f "$dockerfile" docker >/dev/null
  docker run --rm --platform "linux/$arch" \
    -v "$PWD":/src \
    -v silvery-cargo-registry:/usr/local/cargo/registry \
    -v "silvery-$kind-target-$arch":/target \
    -e CARGO_TARGET_DIR=/target \
    -e GGML_NATIVE=OFF \
    "$image" bash -c "$2"
}
