# Release checklist

The procedure that cut v0.1.0, kept current so the next cut repeats it instead of rediscovering it. Artifacts are named `robco-term-<VERSION>-<platform>-<arch>` bare where a user runs them directly, and by each package format's own convention otherwise.

## Before tagging

- [ ] Version in `[workspace.package]` (root `Cargo.toml`) is the target version. Bumping it is its own decision, made ahead of cutting, not during.
- [ ] `debian/changelog` has an entry for the target version.
- [ ] All changes committed and pushed to main.
- [ ] The release commit's CI run is green: it proves the Windows build and produces the exe artifact the release will carry.

## Build the Linux artifacts

The settings window's self-contained image first, then the tarball that requires it, then the package:

```bash
BUILD_DIR=/var/tmp/robco-settings-build bash settings/zipfs/build-selfcontained.sh
cargo run -p xtask -- dist --out-dir dist --settings-binary <path the script printed>
dpkg-buildpackage -us -uc -b
```

`dist/` holds the tarball; the `.deb` and `-dbgsym` `.ddeb` land in the parent directory, dpkg-buildpackage's convention. Point the build script's `BUILD_DIR` at disk, not the RAM-backed `/tmp`. Run `lintian` on the `.changes` before shipping the deb; the accepted findings are the two missing manual pages.

## Fetch the Windows artifact

CI builds `robco-term.exe` (release profile) on every push. Download it from the release commit's run and stamp it:

```bash
gh run download <run-id> -n robco-term-windows-x86_64 -D <dir>
mv <dir>/robco-term.exe <dir>/robco-term-<VERSION>-windows-x86_64.exe
```

Bare, not zipped: the sibling projects (questlog among them) ship single-file executables a user downloads and runs, and the Windows audience this project courts knows that shape from PuTTY itself. The exe is unsigned and meets SmartScreen once; it carries no settings window, which the release notes must say for as long as it is true.

## Publish

```bash
git tag v<VERSION> && git push origin refs/tags/v<VERSION>
gh release create v<VERSION> --title "RobCo Terminal <VERSION>" -F <notes file> \
  dist/robco-term-<VERSION>-linux-x86_64.tar.gz \
  ../robco-term_<VERSION>_amd64.deb \
  ../robco-term-dbgsym_<VERSION>_amd64.ddeb \
  <dir>/robco-term-<VERSION>-windows-x86_64.exe
```

Write the notes for the stranger on the releases page: what each asset is, what it needs, and what is honestly untested. A recut (assets or notes corrected after publish) keeps the version number: `gh release upload`/`delete-asset`/`edit` against the same tag.

## Verify

Read the asset list back and check every artifact above appears:

```bash
gh release view v<VERSION> --json assets --jq '.assets[].name'
```

A release the page does not show is not released, whatever was built.

## Open

questlog's `release-images` workflow builds each platform image in CI and attaches it on publish automatically; adopting that here would replace the manual fetch-and-upload steps above.
