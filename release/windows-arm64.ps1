# Release: Silvery for Windows 11 on ARM PCs (Snapdragon and other ARM64 laptops).
# Run in PowerShell on a Windows ARM64 PC:   .\release\windows-arm64.ps1
# Needs the Visual Studio "C++ Clang Compiler for Windows" component and LLVM for ARM64.
# Result: dist\release\Silvery-Windows-arm64.zip
. (Join-Path $PSScriptRoot "windows-common.ps1")
Build-SilveryWindows -RustTarget "aarch64-pc-windows-msvc" -Label "arm64"
