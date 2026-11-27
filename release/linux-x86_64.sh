#!/bin/bash
# Release: Silvery for Linux on Intel/AMD PCs (x86_64).
# Run on Linux x86_64 (builds directly) or on a Mac/other computer with Docker (builds on Debian 12).
# Result: dist/release/Silvery-Linux-x86_64.tar.gz
source "$(dirname "$0")/common.sh"
exe=$(build_linux x86_64) || exit 1
package_linux x86_64 "$exe"
