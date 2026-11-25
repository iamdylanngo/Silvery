#!/bin/bash
# Kept for convenience: same as ./build-linux.sh test [x86_64|arm64]
exec "$(dirname "$0")/build-linux.sh" test "${1:-x86_64}"
