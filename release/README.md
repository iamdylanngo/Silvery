# Release scripts

There's one script per kind of computer. Each one builds Silvery and puts a ready-to-share download in
`dist/release/`. The GitHub workflow `.github/workflows/release.yml` runs all six and publishes the files on
the Releases page. You can also run any of them yourself.

| Script | For | Run on | Result |
|---|---|---|---|
| `macos-intel.sh` | Macs with an Intel chip | any Mac | `Silvery-macOS-Intel.zip` |
| `macos-apple-silicon.sh` | Macs with an Apple chip (M1 and later) | any Mac | `Silvery-macOS-AppleSilicon.zip` |
| `linux-x86_64.sh` | Linux on Intel/AMD PCs | Linux x86_64, or any computer with Docker | `Silvery-Linux-x86_64.tar.gz` |
| `linux-arm64.sh` | Linux on ARM (Ubuntu on Apple-chip Macs, Raspberry Pi 4/5) | Linux arm64, or any computer with Docker | `Silvery-Linux-arm64.tar.gz` |
| `windows-x64.ps1` | Windows 10/11 on Intel/AMD PCs | a Windows x64 PC (PowerShell) | `Silvery-Windows-x64.zip` |
| `windows-arm64.ps1` | Windows 11 on ARM PCs | a Windows ARM64 PC (PowerShell) | `Silvery-Windows-arm64.zip` |

```sh
./release/macos-apple-silicon.sh     # macOS / Linux (bash)
```
```powershell
.\release\windows-x64.ps1            # Windows (PowerShell)
```

The file names don't contain the version, so the "latest" download links never change. The version
(shown in Silvery's About box) comes from `version` in `Cargo.toml`. GitHub releases set it from the tag. Requirements for each system are in
[docs/building.md](../docs/building.md). Linux builds made with Docker use Debian 12, so they run on
Debian 12+, Ubuntu 22.10+ and most other current Linux systems.
