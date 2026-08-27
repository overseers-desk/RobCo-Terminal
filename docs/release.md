# Release checklist

The procedure that cut v0.1.0, kept current so the next cut repeats it instead of rediscovering it. Artifacts are named `robco-term-<VERSION>-<platform>-<arch>` bare where a user runs them directly, and by each package format's own convention otherwise. The Windows artifact is one bare `.exe`: the settings window and a static Tcl/Tk ride inside the terminal binary, and right-click re-executes that same file with `--settings`. The macOS artifact is a `.tar.gz` holding `robco-term` and `robco-settings` at the archive root, because there the settings window is a second binary the terminal looks for beside its own, as on Linux.

## Before tagging

- [ ] Version in `[workspace.package]` (root `Cargo.toml`) is the target version. Bumping it is its own decision, made ahead of cutting, not during.
- [ ] `debian/changelog` has an entry for the target version.
- [ ] All changes committed and pushed to main.
- [ ] The release commit's CI run is green: `windows` proves the Windows build, including the settings payload's own `--settings-selftest`, and `macos` proves the terminal and the settings image build there. The release's own Windows exe and macOS tarball are built fresh by `release.yml` when the tag is pushed, not carried over from this run.

## Build the Linux artifacts

The settings window's self-contained image first, then the tarball that requires it, then the package:

```bash
BUILD_DIR=/var/tmp/robco-settings-build bash settings/zipfs/build-selfcontained.sh
cargo run -p xtask -- dist --out-dir dist --settings-binary <path the script printed>
dpkg-buildpackage -us -uc -b
```

`dist/` holds the tarball; the `.deb` and `-dbgsym` `.ddeb` land in the parent directory, dpkg-buildpackage's convention. Point the build script's `BUILD_DIR` at disk, not the RAM-backed `/tmp`. Run `lintian` on the `.changes` before shipping the deb; the accepted findings are the two missing manual pages.

## Publish

```bash
git tag v<VERSION> && git push origin refs/tags/v<VERSION>
gh release create v<VERSION> --title "RobCo Terminal <VERSION>" -F <notes file> \
  dist/robco-term-<VERSION>-linux-x86_64.tar.gz \
  ../robco-term_<VERSION>_amd64.deb \
  ../robco-term-dbgsym_<VERSION>_amd64.ddeb
```

The tag push above also triggers `.github/workflows/release.yml`, which carries both non-Linux platforms and waits for the release to exist before uploading, so run `gh release create` promptly after the push rather than long after it. Its `windows` job builds the embedded-settings exe, proves the payload with `--settings-selftest`, and uploads `robco-term-<VERSION>-windows-x86_64.exe`. Its `macos` job builds the terminal and the self-contained settings image on `macos-latest`, runs the image's own `--selftest`, and uploads the pair as `robco-term-<VERSION>-macos-arm64.tar.gz` (Apple Silicon; an Intel or universal build is unbuilt). Both are unsigned: Windows meets SmartScreen once, and macOS refuses the first launch until the user opens it from the context menu or clears the quarantine attribute, which the notes have to say. Nothing to fetch or stamp by hand for either; check the workflow run finished before moving on to Verify below.

Write the notes for the stranger on the releases page: what each asset is, what it needs, and what is honestly untested. A recut (assets or notes corrected after publish) keeps the version number: `gh release upload`/`delete-asset`/`edit` against the same tag.

## Verify

Read the asset list back and check every artifact above appears:

```bash
gh release view v<VERSION> --json assets --jq '.assets[].name'
```

A release the page does not show is not released, whatever was built.

## Open

The Windows and macOS halves are automatic (`.github/workflows/release.yml`). The Linux half above, the tarball and the `.deb`, is still built and uploaded by hand; carrying questlog's `release-images` pattern the rest of the way would automate that half too. On macOS, signing and notarisation, an Intel or universal build, and a `.app` or `.dmg` in place of the tarball are all open.
