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
# The bundle carries no seal of its own, and cannot until the settings
# image changes shape. Sealing a bundle covers every Mach-O inside it, and
# that image is a Tcl interpreter with its script archive appended past the
# end of its Mach-O, which codesign rejects outright ("main executable
# failed strict validation"). What holds the app together instead is the
# ad-hoc signature the linker gave each binary at build time, which is all
# Apple Silicon requires to execute them and which the append leaves
# intact, covering as it does only the code pages.
#
# Gatekeeper wants a Developer ID rather than a seal, so nothing is lost
# today: a downloaded copy is refused on first launch either way. The day
# signing is real, notarization will demand a signature on every Mach-O in
# the bundle, and this file has to become signable first: either its
# archive is unpacked into Contents/Resources beside a plain interpreter,
# or the settings program is embedded in the terminal the way the Windows
# build embeds it, leaving one binary in Contents/MacOS.
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

# What is checked instead of a seal: that both binaries still run from
# where they now sit. This is the whole of what matters and the only form
# the check can take, because Apple Silicon refuses to execute a Mach-O
# whose signature is missing or broken, while codesign asked to verify a
# file inside a bundle demands the resource envelope an unsealed bundle
# does not have. Running them answers the real question; verifying them
# answers a question about a bundle we are not making.
"$app/Contents/MacOS/robco-term" --version > /dev/null
"$app/Contents/MacOS/robco-settings" --version > /dev/null

# The alias is the drag-to-install gesture every Mac user already knows;
# without it the mounted volume shows an app and no hint of what to do.
ln -s /Applications "$stage/Applications"

mkdir -p "$out_dir"
image="$out_dir/robco-term-$version-macos-arm64.dmg"
rm -f "$image"
hdiutil create -volname "RobCo Terminal" -srcfolder "$stage" \
    -fs HFS+ -format UDZO -ov -quiet "$image"
echo "$image"
