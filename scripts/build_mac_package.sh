#!/bin/sh
set -eu

# Tauri's macOS bundler requires Apple's xattr implementation. Put system
# utilities ahead of Python/Ruby shims that may expose incompatible commands.
PATH="/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
export PATH

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
APP_DIR="$ROOT_DIR/apps/desktop"
STAGE_DIR="$APP_DIR/src-tauri/resources/exiftool/macos"
DIST_DIR="$ROOT_DIR/dist"

command -v cargo >/dev/null 2>&1 || {
  echo "ERROR: Rust and Cargo are required." >&2
  exit 1
}
command -v node >/dev/null 2>&1 || {
  echo "ERROR: Node.js is required." >&2
  exit 1
}

BUILD_VERSION=$(node "$SCRIPT_DIR/geotagger_build_version.mjs")
MAC_BUILD_TARGET=${SHUTTERTRAIL_MAC_TARGET:-$(uname -m)}
case "$MAC_BUILD_TARGET" in
  aarch64-*|arm64) MAC_ARCH_LABEL="Apple-Silicon" ;;
  x86_64-*|x86_64) MAC_ARCH_LABEL="Intel" ;;
  universal-apple-darwin) MAC_ARCH_LABEL="Universal" ;;
  *)
    echo "ERROR: Unsupported macOS build architecture: $MAC_BUILD_TARGET" >&2
    exit 1
    ;;
esac
OUTPUT_PACKAGE="$DIST_DIR/ShutterTrail-GeoTagger-$BUILD_VERSION-macOS-$MAC_ARCH_LABEL.dmg"

command -v brew >/dev/null 2>&1 || {
  echo "ERROR: Homebrew is required to stage a redistributable ExifTool layout." >&2
  exit 1
}

EXIFTOOL_PREFIX=$(brew --prefix exiftool 2>/dev/null) || {
  echo "ERROR: Install ExifTool with: brew install exiftool" >&2
  exit 1
}
EXIFTOOL_SCRIPT="$EXIFTOOL_PREFIX/libexec/bin/exiftool"
EXIFTOOL_LIB="$EXIFTOOL_PREFIX/libexec/lib/perl5"
if [ ! -f "$EXIFTOOL_SCRIPT" ] || [ ! -d "$EXIFTOOL_LIB/Image" ] || [ ! -f "$EXIFTOOL_LIB/File/RandomAccess.pm" ]; then
  echo "ERROR: The Homebrew ExifTool layout was not recognized at $EXIFTOOL_PREFIX." >&2
  exit 1
fi

echo "Staging ExifTool..."
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/lib/File"
cp "$EXIFTOOL_SCRIPT" "$STAGE_DIR/exiftool"
cp -R "$EXIFTOOL_LIB/Image" "$STAGE_DIR/lib/Image"
cp "$EXIFTOOL_LIB/File/RandomAccess.pm" "$STAGE_DIR/lib/File/RandomAccess.pm"
# Some Homebrew Perl modules are installed read-only. Keep packaged resources
# writable so Tauri can safely replace a previous local build.
chmod -R u+rwX "$STAGE_DIR"
# Homebrew pins the source script to its current Perl version. Use the macOS
# Perl launcher so the packaged layout remains independent of Homebrew paths.
sed -i.bak '1s|^#!.*$|#!/usr/bin/perl|' "$STAGE_DIR/exiftool"
rm -f "$STAGE_DIR/exiftool.bak"
chmod 755 "$STAGE_DIR/exiftool"

cd "$APP_DIR"
if [ -n "${SHUTTERTRAIL_MAC_TARGET:-}" ]; then
  RELEASE_DIR="$APP_DIR/src-tauri/target/$SHUTTERTRAIL_MAC_TARGET/release"
else
  RELEASE_DIR="$APP_DIR/src-tauri/target/release"
fi
for OLD_OUTPUT in "$RELEASE_DIR/bundle" "$RELEASE_DIR/resources/exiftool"; do
  if [ -d "$OLD_OUTPUT" ]; then
    chmod -R u+w "$OLD_OUTPUT"
  fi
done
if [ -d "$RELEASE_DIR/resources/exiftool" ]; then
  rm -rf "$RELEASE_DIR/resources/exiftool"
fi
echo "Installing locked frontend dependencies..."
npm ci

echo "Running frontend and Rust tests..."
npm run check
cargo test --manifest-path src-tauri/Cargo.toml

echo "Building macOS DMG..."
if [ -z "${APPLE_CERTIFICATE:-}" ]; then
  # Seal the complete app bundle, including ExifTool resources, even for local
  # builds without a Developer ID certificate. Public builds should provide
  # APPLE_CERTIFICATE and Apple's notarization credentials instead.
  SIGNING_CONFIG="src-tauri/tauri.adhoc.conf.json"
else
  SIGNING_CONFIG=""
fi
if [ -n "${SHUTTERTRAIL_MAC_TARGET:-}" ]; then
  if [ -n "$SIGNING_CONFIG" ]; then
    npm run tauri -- build --bundles dmg --target "$SHUTTERTRAIL_MAC_TARGET" --config "$SIGNING_CONFIG"
  else
    npm run tauri -- build --bundles dmg --target "$SHUTTERTRAIL_MAC_TARGET"
  fi
  BUNDLE_ROOT="$APP_DIR/src-tauri/target/$SHUTTERTRAIL_MAC_TARGET/release/bundle/dmg"
else
  if [ -n "$SIGNING_CONFIG" ]; then
    npm run tauri -- build --bundles dmg --config "$SIGNING_CONFIG"
  else
    npm run tauri -- build --bundles dmg
  fi
  BUNDLE_ROOT="$APP_DIR/src-tauri/target/release/bundle/dmg"
fi

echo "Collecting macOS package..."
mkdir -p "$DIST_DIR"
PACKAGE_FOUND=0
for PACKAGE in "$BUNDLE_ROOT"/*_"$BUILD_VERSION"_*.dmg; do
  if [ -f "$PACKAGE" ]; then
    cp "$PACKAGE" "$OUTPUT_PACKAGE"
    PACKAGE_FOUND=1
  fi
done
if [ "$PACKAGE_FOUND" -ne 1 ]; then
  echo "ERROR: No DMG was produced under $BUNDLE_ROOT." >&2
  exit 1
fi

echo
echo "macOS package:"
echo "  $OUTPUT_PACKAGE"
