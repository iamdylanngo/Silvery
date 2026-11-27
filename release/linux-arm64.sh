#!/bin/bash
# Release: Silvery for Linux on ARM (Ubuntu on Apple-chip Macs, Raspberry Pi 4/5, ARM servers).
# Run on Linux arm64 (builds directly) or on a Mac/other computer with Docker (builds on Debian 12).
# Result: dist/release/Silvery-Linux-arm64.tar.gz
source "$(dirname "$0")/common.sh"
exe=$(build_linux arm64) || exit 1
package_linux arm64 "$exe"
