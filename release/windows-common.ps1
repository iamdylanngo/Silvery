# Shared by the Windows release scripts. Not meant to be run on its own.
#
# Needs on the Windows PC:
#   - Rust (https://rustup.rs)
#   - Visual Studio 2022 Build Tools with "Desktop development with C++"
#     (for ARM64 also the "C++ Clang Compiler for Windows" component)
#   - CMake and LLVM:  winget install Kitware.CMake LLVM.LLVM

$ErrorActionPreference = "Stop"

function Build-SilveryWindows {
    param(
        [Parameter(Mandatory)] [string] $RustTarget,   # x86_64-pc-windows-msvc or aarch64-pc-windows-msvc
        [Parameter(Mandatory)] [string] $Label         # x64 or arm64
    )

    $root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
    Set-Location $root

    $version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' |
        Select-Object -First 1).Matches[0].Groups[1].Value
    $out = Join-Path $root "dist\release"
    New-Item -ItemType Directory -Force -Path $out | Out-Null

    foreach ($tool in "cargo", "cmake") {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw "$tool is missing. See docs/building.md (Windows)."
        }
    }

    # The speech engine's Rust bindings are generated with LLVM's libclang.
    if (-not $env:LIBCLANG_PATH) {
        foreach ($dir in "C:\Program Files\LLVM\bin", "C:\Program Files (x86)\LLVM\bin") {
            if (Test-Path (Join-Path $dir "libclang.dll")) { $env:LIBCLANG_PATH = $dir; break }
        }
    }
    if (-not $env:LIBCLANG_PATH) {
        Write-Warning "LLVM (libclang.dll) not found. Install it with: winget install LLVM.LLVM"
    }

    Write-Host "==> Building Silvery $version for Windows $Label"
    rustup target add $RustTarget
    if ($LASTEXITCODE -ne 0) { throw "rustup failed" }

    # Downloads must run on other people's PCs: don't tune the speech engine to this PC's processor,
    # and keep these builds in their own folder.
    $env:GGML_NATIVE = "OFF"
    $env:CARGO_TARGET_DIR = Join-Path $root "target\release-portable"

    # Put the C/C++ runtime inside the .exe, so it runs without installing anything else.
    $env:RUSTFLAGS = "-C target-feature=+crt-static"
    if ($RustTarget -like "aarch64-*") {
        # The speech engine can't be compiled for ARM with Microsoft's compiler; use Clang instead.
        $env:CMAKE_GENERATOR_TOOLSET = "ClangCL"
    }

    cargo build --release --target $RustTarget
    if ($LASTEXITCODE -ne 0) { throw "The build failed." }

    $name = "Silvery-$version-Windows-$Label"
    $stage = Join-Path ([System.IO.Path]::GetTempPath()) $name
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Path $stage | Out-Null

    Copy-Item (Join-Path $env:CARGO_TARGET_DIR "$RustTarget\release\silvery.exe") (Join-Path $stage "Silvery.exe")
    Copy-Item "LICENSE" (Join-Path $stage "LICENSE.txt")
    @"
Silvery $version: private meeting notes with a local AI

1. Move this folder somewhere you like (for example Documents) and double-click Silvery.exe.
   If Windows shows "Windows protected your PC", choose More info > Run anyway (first time only).
2. Install Ollama from https://ollama.com and download a model, for example:  ollama pull qwen3:8b
3. If no sound comes in when recording: Settings > Privacy & security > Microphone,
   turn on "Let desktop apps access your microphone".

User guide: inside Silvery, Help > Silvery User Guide.
"@ | Set-Content -Encoding UTF8 (Join-Path $stage "README.txt")

    $zip = Join-Path $out "$name.zip"
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip
    Remove-Item -Recurse -Force $stage
    Write-Host "==> Made $zip"
}
