# Publishing a Silvery release on GitHub

People download Silvery from the repository's **Releases** page. `.github/workflows/release.yml` builds the
releases automatically on GitHub's own Mac, Linux and Windows machines, using the scripts in
[`release/`](../release/README.md). You don't upload files by hand.

## What a release contains

| File | For | Made by |
|---|---|---|
| `Silvery-<version>-macOS-AppleSilicon.zip` | Macs with an Apple chip (M1+), macOS 11+ | `release/macos-apple-silicon.sh` |
| `Silvery-<version>-macOS-Intel.zip` | Macs with an Intel chip, macOS 11+ | `release/macos-intel.sh` |
| `Silvery-<version>-Linux-x86_64.tar.gz` | Linux on Intel/AMD PCs | `release/linux-x86_64.sh` |
| `Silvery-<version>-Linux-arm64.tar.gz` | Linux on ARM | `release/linux-arm64.sh` |
| `Silvery-<version>-Windows-x64.zip` | Windows 10/11 on Intel/AMD PCs | `release/windows-x64.ps1` |
| `Silvery-<version>-Windows-arm64.zip` | Windows 11 on ARM | `release/windows-arm64.ps1` |
| `SHA256SUMS.txt` | Checksums for checking the downloads | the workflow |

- The **Mac** zips contain `Silvery.app` with its icon and the microphone description macOS needs.
- The **Linux** archives contain the program, its icon, an applications-menu entry and `install.sh`.
  On GitHub they're built on Ubuntu 22.04, so they run on Ubuntu 22.04+, Debian 12+ and similar.
- The **Windows** zips contain `Silvery.exe` (with its icon and the C++ runtime built in), `README.txt`
  and the license.
- Every download is built for its kind of processor in general, not for the build machine, so it runs on
  older and newer computers alike.

## Publish a new version

1. Update `version` in `Cargo.toml` (for example `0.2.0`) and commit it. The file names use this number.
2. Tag the commit and push the tag:
   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. Open the repository's **Actions** tab and watch **Release**: six build jobs, then **Publish the release**.
   It takes about 15–25 minutes.
4. When it's done, the new version is on the **Releases** page, with release notes generated from the
   commits since the last version. You can edit the text there afterwards.

**Without the command line:** Actions › **Release** › **Run workflow**, enter a tag such as `v0.2.0` and
press **Run workflow**. The tag is created for you.

The link that always points to the newest version is `https://github.com/<owner>/<repo>/releases/latest`.
The README's Download section already links there.

## If a build fails

- **Windows arm64** is marked *experimental*. If it fails, the release still goes out with the other five
  files. Open that job's log to see why. It needs Clang and LLVM for ARM, which the workflow installs.
- If any other job fails, **Publish** still runs and posts whatever was built. Fix the problem, then run
  **Release** again with the same tag. The files are updated in place.

## Making a release by hand

Run the scripts yourself (each only on a computer that can build it; see the table in
[`release/README.md`](../release/README.md)). Then create a release on GitHub and drag the files from
`dist/release/` into it. From a Mac with Docker you can make four of the six: both Macs and both Linux.
`./release/test-linux.sh arm64` (or `x86_64`) tries a Linux download on Debian 12.

## Good to know

- **Private repositories:** only people with access can see the releases. GitHub's ARM machines
  (`ubuntu-22.04-arm`, `windows-11-arm`) are free for public repositories; private repositories may need a
  paid plan for them. If those jobs can't start, remove them from `release.yml`.
- **Settings › Actions › General › Workflow permissions** must allow "Read and write", so the workflow can
  create the release.
- **Signing:** the Mac app is signed ad-hoc only, so people open it the first time with right-click › Open.
  Windows shows "Windows protected your PC" until the program is signed. Removing these warnings needs
  an Apple Developer account ($99/year, plus notarization) and a Windows code-signing certificate.
