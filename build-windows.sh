#!/bin/bash
# Builds Silvery for Windows 10/11 (64-bit) inside Docker, cross-compiled with MinGW-w64.
# STATUS: paused / unfinished. The .exe builds, but it still needs libstdc++-6.dll next to it and has
# not been tried on Windows yet, so the result goes to dist/windows-unfinished/.
#   ./build-windows.sh
set -euo pipefail
cd "$(dirname "$0")"
source docker/run.sh
mkdir -p dist/windows-unfinished

echo "=== Windows (x86_64) — unfinished ==="
# bindgen reads the Windows headers from MinGW.
# The speech engine asks for the C++ runtime as a DLL; offer the static one first so it ends up inside the .exe.
run amd64 'export BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_gnu="--target=x86_64-w64-mingw32 --sysroot=/usr/x86_64-w64-mingw32 -I/usr/x86_64-w64-mingw32/include";
     mkdir -p /tmp/static-cxx && cp "$(x86_64-w64-mingw32-g++ -print-file-name=libstdc++.a)" /tmp/static-cxx/;
     export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-L native=/tmp/static-cxx -C link-arg=-static -C link-arg=-static-libgcc -C link-arg=-static-libstdc++";
     build() { cargo build --release --target x86_64-pc-windows-gnu; }
     # MinGW names the speech-engine libraries "ggml.a"; Rust looks for "libggml.a". Add those names, then link.
     if ! build; then
       find /target/x86_64-pc-windows-gnu/release/build -path "*whisper-rs-sys*" -name "*.a" ! -name "lib*" |
         while read -r f; do ln -sf "$f" "$(dirname "$f")/lib$(basename "$f")"; done
       build
     fi &&
     cp /target/x86_64-pc-windows-gnu/release/silvery.exe /src/dist/windows-unfinished/silvery-windows-x86_64.exe'

ls -la dist/windows-unfinished
