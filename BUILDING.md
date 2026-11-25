# Building Silvery on macOS, Windows and Linux

Silvery's speech-to-text (Whisper) runs on the computer itself on all three systems.
On Macs it also uses the graphics chip (Metal) automatically. On Windows and Linux it uses the processor;
a computer with a graphics card can use a faster build (see below).

Every system needs Rust (https://rustup.rs) and **Ollama** (https://ollama.com) with a model, for example
`ollama pull qwen3:8b`. Silvery tries to start Ollama by itself.

## macOS: `./build-macos.sh`
1. Install Xcode Command Line Tools (`xcode-select --install`) and CMake (`brew install cmake`).
2. Run one of:
   - `./build-macos.sh`: for this Mac's chip
   - `./build-macos.sh arm64`: Apple-chip Macs (M1 and later)
   - `./build-macos.sh intel`: Intel Macs
   - `./build-macos.sh universal`: one app for both kinds of Mac
3. You get `dist/Silvery.app` (double-click it) and `dist/Silvery-macos-<kind>.zip` to send to other Macs.
   The app includes the microphone description, so macOS asks once for microphone access.
   Needs macOS 11 or newer.
4. On another Mac, the first time: right-click Silvery.app › Open › Open. The app is signed only
   "ad-hoc" (no Apple developer certificate), so macOS warns that it's from an unidentified developer.

## Windows 10/11
1. Install "Visual Studio Build Tools" with **Desktop development with C++**, plus CMake and LLVM
   (`winget install Kitware.CMake LLVM.LLVM`).
2. `cargo build --release` → `target\release\silvery.exe`
3. If no sound comes in: Settings › Privacy & security › Microphone, turn on
   "Let desktop apps access your microphone".

## Linux (Ubuntu/Debian)
1. `sudo apt install build-essential cmake clang libclang-dev pkg-config libasound2-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev`
2. `cargo build --release` → `target/release/silvery`

## Faster speech-to-text with a graphics card (Windows/Linux)
- Nvidia (CUDA toolkit installed): `cargo build --release --features cuda`
- Other cards (Vulkan SDK installed): `cargo build --release --features vulkan`

## Building the Linux and Windows versions from a Mac (Docker)
Docker Desktop must be running. There are two scripts:

**Linux: `./build-linux.sh`** builds both Linux versions into `dist/`:
- `silvery-linux-x86_64`: for 64-bit Intel/AMD Linux PCs (`./build-linux.sh x86_64` for just this one).
- `silvery-linux-arm64`: for ARM Linux, such as Ubuntu in UTM/Parallels on an Apple-chip Mac,
  Raspberry Pi 4/5 or ARM servers (`./build-linux.sh arm64` for just this one).
- `./build-linux.sh test x86_64` or `… test arm64` tries a build: speech-to-text on a test clip
  and a picture of the window.

Both need the usual desktop libraries, which ordinary Ubuntu desktops already have:
`sudo apt install libasound2t64 libxkbcommon-x11-0 libgl1` (`libasound2` on Ubuntu 22.04 or older).

**Windows: `./build-windows.sh`** is **paused / unfinished**. It produces
`dist/windows-unfinished/silvery-windows-x86_64.exe`, which still needs `libstdc++-6.dll` beside it and
hasn't been tried on Windows yet.

## Automatic builds
`.github/workflows/build.yml` builds all three versions on GitHub's own Mac, Windows and Linux machines
whenever this folder is pushed to a GitHub repository; the programs appear under the run's "Artifacts".

## Where Silvery keeps things
- macOS: `~/Library/Application Support/Silvery`
- Windows: `%APPDATA%\Silvery`
- Linux: `~/.local/share/Silvery`
