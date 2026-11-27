# Publishing a Silvery release on GitHub

People download Silvery from the repository's **Releases** page. `.github/workflows/release.yml` builds the
releases automatically on GitHub's own Mac, Linux and Windows machines, using the scripts in
[`release/`](../release/README.md). You don't upload files by hand.

## What a release contains

| File | For | Made by |
|---|---|---|
| `Silvery-macOS-AppleSilicon.zip` | Macs with an Apple chip (M1+), macOS 11+ | `release/macos-apple-silicon.sh` |
| `Silvery-macOS-Intel.zip` | Macs with an Intel chip, macOS 11+ | `release/macos-intel.sh` |
| `Silvery-Linux-x86_64.tar.gz` | Linux on Intel/AMD PCs | `release/linux-x86_64.sh` |
| `Silvery-Linux-arm64.tar.gz` | Linux on ARM | `release/linux-arm64.sh` |
| `Silvery-Windows-x64.zip` | Windows 10/11 on Intel/AMD PCs | `release/windows-x64.ps1` |
| `Silvery-Windows-arm64.zip` | Windows 11 on ARM | `release/windows-arm64.ps1` |
| `SHA256SUMS.txt` | Checksums for checking the downloads | the workflow |

- The **Mac** zips contain `Silvery.app` with its icon and the microphone description macOS needs.
- The **Linux** archives contain the program, its icon, an applications-menu entry and `install.sh`.
  On GitHub they're built on Ubuntu 22.04, so they run on Ubuntu 22.04+, Debian 12+ and similar.
- The **Windows** zips contain `Silvery.exe` (with its icon and the C++ runtime built in), `README.txt`
  and the license.
- Every download is built for its kind of processor in general, not for the build machine, so it runs on
  older and newer computers alike.

## Publish a new version

1. Pick the new version number, for example `1.2.0`. The release takes it from the tag (`v1.2.0` → `1.2.0`)
   and shows it in Silvery's About box. You don't have to edit `Cargo.toml`, but you can keep it in step.
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

**Keeping the download page clear**
- The files have the same names in every release (no version number in them). So
  `https://github.com/<owner>/<repo>/releases/latest/download/Silvery-macOS-AppleSilicon.zip` (and the same
  for the other five) always downloads the newest version. The README's Download buttons use these links.
- Each release has exactly the six downloads plus `SHA256SUMS.txt`. Running the release again for the same
  tag replaces the files and removes any old leftovers.
- The newest release is marked **Latest**, and its notes start with a "Download Silvery" table of direct links.
- GitHub always lists older releases too, which is useful for going back to an older version. To hide one,
  open it on the Releases page and delete it (its tag can stay). People following the README or the
  "latest" links never see the old ones.

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
- **Signing:** without the Mac signing secrets below, the Mac app is signed ad-hoc only, so macOS says
  *"Apple could not verify…"* and people use **System Settings › Privacy & Security › Open Anyway** once.
  Windows shows "Windows protected your PC" until the program is signed. Removing these warnings needs
  an Apple Developer account ($99/year, plus notarization) and a Windows code-signing certificate.

## Removing the macOS warning: Developer ID signing and notarization

macOS only opens downloaded apps without a warning if they're signed with an Apple **Developer ID** and
**notarized** (checked by Apple). The release workflow and `build-macos.sh` do both automatically as soon as
the details below exist. Until then the Mac downloads still work, with the one-time *Open Anyway* step.

**1. Get the certificate (once).**
- Join the Apple Developer Program at developer.apple.com ($99/year).
- In Xcode › Settings › Accounts › *Manage Certificates*, add a **Developer ID Application** certificate
  (or create it at developer.apple.com › Certificates).
- In the Keychain Access app, right-click the certificate › **Export…** as a `.p12` file with a password.
- Its full name (e.g. `Developer ID Application: Jane Doe (AB12CD34EF)`) is the *signing identity*.
  `AB12CD34EF` is your *Team ID*.

**2. Make an app-specific password** at account.apple.com › Sign-In and Security › App-Specific Passwords.

**3. Add these repository secrets** (GitHub › Settings › Secrets and variables › Actions › New secret):

| Secret | Value |
|---|---|
| `MACOS_CERTIFICATE` | the `.p12` file as base64: `base64 -i certificate.p12 \| pbcopy`, then paste |
| `MACOS_CERTIFICATE_PASSWORD` | the password you chose when exporting the `.p12` |
| `MACOS_SIGN_IDENTITY` | `Developer ID Application: Jane Doe (AB12CD34EF)` |
| `APPLE_ID` | the Apple ID email of the developer account |
| `APPLE_TEAM_ID` | `AB12CD34EF` |
| `APPLE_APP_PASSWORD` | the app-specific password from step 2 |

The next release then signs Silvery with the hardened runtime (plus permission to use the microphone, from
`assets/Silvery.entitlements`), sends it to Apple for notarization, and staples the result to the app.
The job log shows *"Notarized: macOS will open it without warnings."*

**On your own Mac**, the same works with environment variables:
```sh
SILVERY_SIGN_IDENTITY="Developer ID Application: Jane Doe (AB12CD34EF)" \
SILVERY_NOTARY_APPLE_ID=you@example.com SILVERY_NOTARY_TEAM_ID=AB12CD34EF \
SILVERY_NOTARY_PASSWORD=abcd-efgh-ijkl-mnop ./release/macos-apple-silicon.sh
```
