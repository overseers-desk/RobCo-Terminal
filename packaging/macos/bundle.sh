#!/usr/bin/env bash
#
# Assemble RobCo Terminal.app around a built terminal and settings image,
# and wrap it in the disk image macOS expects a download to be.
#
# The two binaries travel together for the same reason they do everywhere
# else: the terminal spawns the settings window by looking beside its own
# executable, so `robco-settings` sits next to `robco-term` in
# Contents/MacOS and is found with no path knowledge at all. What the
# bundle adds over a folder of two files is what macOS reads out of
# Info.plist -- a name in the Dock and the menu bar, an icon, a version
# the Finder can show -- none of which a bare Unix binary has.
#
# The signature here is ad-hoc (`-s -`), which is not a Developer ID and
# does not satisfy Gatekeeper: a downloaded copy is still quarantined and
# still refused on first launch. It is here because Apple Silicon refuses
# to execute a binary with no signature at all, and because a bundle whose
# seal does not match its contents is reported as damaged rather than as
# unsigned, which is a worse thing to hand a stranger.
set -euo pipefail

terminal=""
settings=""
version=""
out_dir="dist"

while [ $# -gt 0 ]; do
    case "$1" in
        --terminal) terminal="$2"; shift 2 ;;
        --settings) settings="$2"; shift 2 ;;
        --version)  version="$2";  shift 2 ;;
        --out-dir)  out_dir="$2";  shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

for required in terminal settings version; do
    if [ -z "${!required}" ]; then
        echo "--$required is required" >&2
        exit 2
    fi
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
icon="$repo_root/packaging/icons/robco-term.icns"
test -f "$icon"

# A staging directory rather than the output directory itself: the disk
# image is built from a folder whose entire contents become the mounted
# volume, so anything else living there would ship inside it.
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
app="$stage/RobCo Terminal.app"

mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$terminal" "$app/Contents/MacOS/robco-term"
cp "$settings" "$app/Contents/MacOS/robco-settings"
chmod +x "$app/Contents/MacOS/robco-term" "$app/Contents/MacOS/robco-settings"
cp "$icon" "$app/Contents/Resources/robco-term.icns"

# 11.0 is where Apple Silicon starts, and the deployment target the Rust
# toolchain builds this binary against; claiming anything older would be a
# promise nobody has tested.
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>RobCo Terminal</string>
	<key>CFBundleExecutable</key>
	<string>robco-term</string>
	<key>CFBundleIconFile</key>
	<string>robco-term</string>
	<key>CFBundleIdentifier</key>
	<string>com.github.overseers-desk.robco-term</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>RobCo Terminal</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$version</string>
	<key>CFBundleVersion</key>
	<string>$version</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
PLIST

# The nested binary first, then the bundle: sealing the bundle records
# what its contents hash to, so anything signed afterwards invalidates it.
codesign --force --sign - "$app/Contents/MacOS/robco-settings"
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"

# The alias is the drag-to-install gesture every Mac user already knows;
# without it the mounted volume shows an app and no hint of what to do.
ln -s /Applications "$stage/Applications"

mkdir -p "$out_dir"
image="$out_dir/robco-term-$version-macos-arm64.dmg"
rm -f "$image"
hdiutil create -volname "RobCo Terminal" -srcfolder "$stage" \
    -fs HFS+ -format UDZO -ov -quiet "$image"
echo "$image"
