#!/bin/bash
# Shared helper for the build scripts: run <amd64|arm64> '<command>' in the Docker build machine
# for that kind of processor. The app folder is mounted at /src; build output stays in a Docker volume.
run() {
  local arch="$1"
  local image="silvery-builder-$arch"
  docker build -q --build-arg PLATFORM="linux/$arch" -t "$image" docker >/dev/null
  docker run --rm --platform "linux/$arch" \
    -v "$PWD":/src \
    -v silvery-cargo-registry:/usr/local/cargo/registry \
    -v "silvery-target-$arch":/target \
    -e CARGO_TARGET_DIR=/target \
    "$image" bash -c "$2"
}
