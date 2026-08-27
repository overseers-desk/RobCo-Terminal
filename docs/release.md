# Release checklist

The procedure that cut v0.1.0, kept current so the next cut repeats it instead of rediscovering it. Artifacts are named `robco-term-<VERSION>-<platform>-<arch>` bare where a user runs them directly, and by each package format's own convention otherwise: a `.zip` on Windows, because the settings window ships as a second exe, `robco-settings.exe`, that the terminal looks for beside its own binary by that fixed name (`crates/app/src/window.rs`, `SETTINGS_BINARY`); a bare download would count on a stranger to put both files in the same folder unprompted.

## Before tagging

- [ ] Version in `[workspace.package]` (root `Cargo.toml`) is the target version. Bumping it is its own decision, made ahead of cutting, not during.
- [ ] `debian/changelog` has an entry for the target version.
- [ ] All changes committed and pushed to main.
- [ ] The release commit's CI run is green: `windows` and `settings-windows` prove both Windows halves build. The release's own Windows zip is built fresh by `release.yml` when the tag is pushed, not carried over from this run.

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

The tag push above also triggers `.github/workflows/release.yml`: it builds `robco-term.exe` and `robco-settings.exe`, zips the pair as `robco-term-<VERSION>-windows-x86_64.zip`, and uploads that to this same release once one exists, so run `gh release create` promptly after the push rather than long after it. The pair is unsigned and meets SmartScreen once. Nothing to fetch or stamp by hand for Windows anymore; check the workflow run finished before moving on to Verify below.

Write the notes for the stranger on the releases page: what each asset is, what it needs, and what is honestly untested. A recut (assets or notes corrected after publish) keeps the version number: `gh release upload`/`delete-asset`/`edit` against the same tag.

## Verify

Read the asset list back and check every artifact above appears:

```bash
gh release view v<VERSION> --json assets --jq '.assets[].name'
```

A release the page does not show is not released, whatever was built.

## Open

The Windows half is automatic now (`.github/workflows/release.yml`). The Linux half above, the tarball and the `.deb`, is still built and uploaded by hand; carrying questlog's `release-images` pattern the rest of the way would automate that half too.
