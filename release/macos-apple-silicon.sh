#!/bin/bash
# Release: Silvery for Apple-chip Macs (M1 and later, macOS 11+).
# Run on any Mac.
# Result: dist/release/Silvery-macOS-AppleSilicon.zip
source "$(dirname "$0")/common.sh"
package_macos arm64 AppleSilicon
