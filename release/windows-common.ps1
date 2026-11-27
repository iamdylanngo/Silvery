# Shared by the Windows release scripts. Not meant to be run on its own.
#
# Needs on the Windows PC:
#   - Rust (https://rustup.rs)
#   - Visual Studio 2022 Build Tools with "Desktop development with C++"
#     (for ARM64 also "MSVC ARM64 build tools" and "C++ CMake tools for Windows", plus LLVM for ARM64)
#   - CMake and LLVM:  winget install Kitware.CMake LLVM.LLVM

$ErrorActionPreference = "Stop"

# Loads the Visual Studio developer environment (compiler, Windows SDK, Ninja) for the given processor,
# like opening "Developer PowerShell for VS". Does nothing if it's already loaded.
function Enter-VsDevEnvironment {
    param([Parameter(Mandatory)] [string] $Arch)   # arm64 or amd64
    if ($env:VSCMD_ARG_TGT_ARCH -eq $Arch) { return }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        Write-Warning "Visual Studio wasn't found; continuing without its developer environment."
        return
    }
    $vs = & $vswhere -latest -products * -property installationPath
    $vcvars = Join-Path $vs "VC\Auxiliary\Build\vcvarsall.bat"
    if (-not (Test-Path $vcvars)) {
        Write-Warning "vcvarsall.bat wasn't found in $vs; install 'Desktop development with C++'."
        return
    }
    $hostArch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "amd64" }
    $spec = if ($hostArch -eq $Arch) { $Arch } else { "${hostArch}_$Arch" }
    Write-Host "==> Loading the Visual Studio developer environment ($spec)"
    $vars = cmd /c "`"$vcvars`" $spec >nul 2>&1 && set"
    if ($LASTEXITCODE -ne 0) { throw "vcvarsall.bat $spec failed. Is the ARM64 C++ build tools component installed?" }
    foreach ($line in $vars) {
        if ($line -match '^([^=]+)=(.*)$') { Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2] }
    }
}

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
        # The speech engine can't be compiled for ARM with Microsoft's own compiler (cl.exe), and the
        # Visual Studio project builder can't be switched to Clang from here (it always adds
        # "host=x64"). So, like whisper.cpp's own Windows-on-ARM build, use Ninja and Clang with
        # release/windows-arm64-clang.cmake, inside a Visual Studio developer environment for ARM64.
        Remove-Item Env:CMAKE_GENERATOR_TOOLSET -ErrorAction SilentlyContinue
        Enter-VsDevEnvironment -Arch "arm64"
        # Use the same LLVM as libclang (on ARM PCs that must be the ARM64 build of LLVM).
        foreach ($dir in @($env:LIBCLANG_PATH, "C:\Program Files\LLVM\bin", "C:\Program Files (x86)\LLVM\bin")) {
            if ($dir -and (Test-Path (Join-Path $dir "clang.exe"))) {
                $env:PATH = "$dir;$env:PATH"
                break
            }
        }
        foreach ($tool in "clang", "clang++", "ninja") {
            if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
                throw "$tool is missing. Install LLVM for ARM64 (winget install LLVM.LLVM) and the Visual Studio 'C++ CMake tools for Windows' component."
            }
        }
        $toolchain = (Join-Path $PSScriptRoot "windows-arm64-clang.cmake") -replace '\\', '/'
        $env:CMAKE_GENERATOR = "Ninja"
        $env:CMAKE_TOOLCHAIN_FILE = $toolchain
        # The C/C++ helper used by Rust crates (and for the flags passed to CMake) must be Clang too.
        $env:CC_aarch64_pc_windows_msvc = "clang"
        $env:CXX_aarch64_pc_windows_msvc = "clang++"
        $env:CFLAGS_aarch64_pc_windows_msvc = "-fms-runtime-lib=static"
        $env:CXXFLAGS_aarch64_pc_windows_msvc = "-fms-runtime-lib=static"

        # A speech-engine build left over from a different setup can't be reused: start it fresh.
        $buildDir = Join-Path $env:CARGO_TARGET_DIR "$RustTarget\release\build"
        $stale = Get-ChildItem -Path $buildDir -Recurse -Filter "CMakeCache.txt" -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -like "*whisper-rs-sys*" -and -not (Select-String -Path $_.FullName -Pattern "windows-arm64-clang.cmake" -SimpleMatch -Quiet) }
        if ($stale) {
            Write-Host "==> Clearing an old speech-engine build"
            cargo clean --release --target $RustTarget -p whisper-rs-sys
        }
    }

    # Build, keeping a copy of the output so the first real errors can be shown at the end.
    $log = Join-Path ([System.IO.Path]::GetTempPath()) "silvery-build-$Label.log"
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"   # compiler messages on stderr aren't failures by themselves
    & cargo build --release --target $RustTarget 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log
    $code = $LASTEXITCODE
    $ErrorActionPreference = $previous
    if ($code -ne 0) {
        $errors = Select-String -Path $log -Pattern '(^|\s)(error|fatal error)(\[|:)|error C\d{4}|error LNK\d{4}|CMake Error' |
            Select-Object -First 25
        Write-Host ""
        Write-Host "==== First errors from the build ===="
        foreach ($e in $errors) {
            $line = $e.Line.Trim()
            Write-Host $line
            # Shown as annotations on GitHub (visible on the run's summary page).
            if ($env:GITHUB_ACTIONS) { Write-Host "::error title=Windows $Label build::$line" }
        }
        throw "The build failed (see 'First errors from the build' above)."
    }

    $name = "Silvery-Windows-$Label"
    $stage = Join-Path ([System.IO.Path]::GetTempPath()) $name
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Path $stage | Out-Null

    Copy-Item (Join-Path $env:CARGO_TARGET_DIR "$RustTarget\release\silvery.exe") (Join-Path $stage "Silvery.exe")
    Copy-Item "LICENSE" (Join-Path $stage "LICENSE.txt")
    @"
Silvery ${version}: private meeting notes with a local AI

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
