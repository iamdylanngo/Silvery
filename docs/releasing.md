# Publishing a Silvery release on GitHub

People download Silvery from the repository's **Releases** page. Releases are built automatically by
`.github/workflows/release.yml`. You don't upload files by hand.

## What a release contains

| File | For |
|---|---|
| `Silvery-macOS.zip` | Macs with an Apple chip or Intel, macOS 11+ (one universal app) |
| `silvery-linux-x86_64.tar.gz` | Linux on Intel/AMD PCs (built on Ubuntu 22.04, so it also runs on Debian 12+) |
| `silvery-linux-arm64.tar.gz` | Linux on ARM (Ubuntu on Apple-chip Macs, Raspberry Pi 4/5, ARM servers) |
| `SHA256SUMS.txt` | Checksums for checking the downloads |

Each Linux archive holds the program, its icon, a menu entry and `install.sh`.
Windows is paused and isn't included yet.

## Publish a new version

1. Update `version` in `Cargo.toml` (for example `0.2.0`) and commit it.
2. Tag the commit and push the tag:
   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. Open the repository's **Actions** tab and watch **Release**. It takes about 10–15 minutes.
4. When it's done, the new version is on the **Releases** page, with release notes generated from the
   commits since the last version. You can edit the text there afterwards.

**Without the command line:** Actions › **Release** › **Run workflow**, enter a tag such as `v0.2.0`, and
press **Run workflow**. The tag is created for you.

The download link that always points to the newest version is `https://github.com/<owner>/<repo>/releases/latest`.
The README's Download section already links there.

## Good to know

- **Private repositories:** only people with access can see the releases. The Linux ARM build uses GitHub's
  `ubuntu-22.04-arm` machines, which are free for public repositories; private repositories may need a paid
  plan for them. If that job can't start, remove the `arm64` line from `release.yml`.
- **Settings › Actions › General › Workflow permissions** must allow "Read and write", so the workflow can
  create the release (the workflow asks for `contents: write`).
- **macOS signing:** the app is signed ad-hoc only, so people open it the first time with right-click › Open.
  Removing that step requires an Apple Developer account ($99/year) and notarization.
- Every push also runs `.github/workflows/build.yml`, which checks that macOS and both Linux versions
  still build. Its programs are under each run's **Artifacts**, which is handy for testing before a release.
