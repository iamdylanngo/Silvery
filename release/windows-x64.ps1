# Release: Silvery for Windows 10/11 on Intel/AMD PCs (x64).
# Run in PowerShell on a Windows x64 PC:   .\release\windows-x64.ps1
# Result: dist\release\Silvery-<version>-Windows-x64.zip
. (Join-Path $PSScriptRoot "windows-common.ps1")
Build-SilveryWindows -RustTarget "x86_64-pc-windows-msvc" -Label "x64"
