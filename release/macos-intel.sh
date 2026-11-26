#!/bin/bash
# Release: Silvery for Intel Macs (macOS 11+).
# Run on any Mac (Apple-chip Macs build the Intel version too).
# Result: dist/release/Silvery-<version>-macOS-Intel.zip
source "$(dirname "$0")/common.sh"
package_macos intel Intel
