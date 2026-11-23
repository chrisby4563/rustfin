#!/usr/bin/env bash
# Builds Rustfin for Windows and packs it into an NSIS installer.
#
# Run from an MSYS2 UCRT64 shell in the project root:
#   packaging/windows/build-installer.sh
#
# Needed packages (pacman -S --needed):
#   git zip mingw-w64-ucrt-x86_64-{toolchain,rust,meson,ninja,pkgconf,gtk4,
#   libadwaita,mpv,libepoxy,nsis,adwaita-icon-theme,librsvg,ntldd}
set -euo pipefail

[[ "${MSYSTEM:-}" == "UCRT64" ]] || { echo "Run this in an MSYS2 UCRT64 shell" >&2; exit 1; }

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
WIN="$ROOT/packaging/windows"
BUILD="${BUILD:-$ROOT/_build-win}"
STAGE="$ROOT/_dist-win/Rustfin"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/delfin/Cargo.toml" | head -1)
MINGW=/ucrt64

echo "==> Building Rustfin $VERSION"
cd "$ROOT"
[[ -d "$BUILD" ]] || meson setup "$BUILD" --prefix="$STAGE" -Dprofile=release
meson compile -C "$BUILD"

echo "==> Staging into $STAGE"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/share/rustfin"
cp "$BUILD/delfin/rustfin" "$STAGE/bin/rustfin.exe" 2>/dev/null \
  || cp "$BUILD/delfin/release/rustfin.exe" "$STAGE/bin/rustfin.exe"
cp "$BUILD/data/resources.gresource" "$STAGE/share/rustfin/"

# All DLLs from the MSYS2 prefix that the app (and its plugins) need.
copy_deps() {
  ntldd -R "$@" 2>/dev/null \
    | sed -n 's/.*=> \(.*\.dll\) .*/\1/Ip' \
    | tr '\\' '/' \
    | while read -r dll; do
        dll=$(cygpath -u "$dll")
        case "$dll" in
          "$MINGW"/bin/*) cp -n "$dll" "$STAGE/bin/" ;;
        esac
      done
}

# gdk-pixbuf loaders (PNG/JPEG/SVG) – thumbnails, posters, icons.
PIXBUF_DIR=$(ls -d $MINGW/lib/gdk-pixbuf-2.0/2.10.0)
mkdir -p "$STAGE/lib/gdk-pixbuf-2.0/2.10.0/loaders"
cp "$PIXBUF_DIR"/loaders/*.dll "$STAGE/lib/gdk-pixbuf-2.0/2.10.0/loaders/"

# GIO modules are optional at runtime; leave them out (reqwest uses SChannel).

copy_deps "$STAGE/bin/rustfin.exe" "$STAGE"/lib/gdk-pixbuf-2.0/2.10.0/loaders/*.dll
# GTK helper processes spawned at runtime
for helper in gdbus.exe gspawn-win64-helper.exe gspawn-win64-helper-console.exe; do
  [[ -f "$MINGW/bin/$helper" ]] && cp "$MINGW/bin/$helper" "$STAGE/bin/"
done

# Loader cache with relative paths (gdk-pixbuf resolves them next to the DLL).
(cd "$STAGE" && GDK_PIXBUF_MODULEDIR=lib/gdk-pixbuf-2.0/2.10.0/loaders \
  gdk-pixbuf-query-loaders > lib/gdk-pixbuf-2.0/2.10.0/loaders.cache.tmp)
sed "s|$(cygpath -m "$STAGE")/||g; s|$STAGE/||g" \
  "$STAGE/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache.tmp" \
  > "$STAGE/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"
rm "$STAGE/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache.tmp"

# GSettings schemas (GTK file chooser, color scheme, …)
mkdir -p "$STAGE/share/glib-2.0/schemas"
cp $MINGW/share/glib-2.0/schemas/org.gtk.*.xml "$STAGE/share/glib-2.0/schemas/" 2>/dev/null || true
cp $MINGW/share/glib-2.0/schemas/gschema.dtd "$STAGE/share/glib-2.0/schemas/" 2>/dev/null || true
glib-compile-schemas "$STAGE/share/glib-2.0/schemas"

# Icon themes: Adwaita (GTK/libadwaita UI icons) + hicolor (fallback)
mkdir -p "$STAGE/share/icons"
cp -r $MINGW/share/icons/Adwaita "$STAGE/share/icons/"
cp -r $MINGW/share/icons/hicolor "$STAGE/share/icons/"
mkdir -p "$STAGE/share/icons/hicolor/scalable/apps"
cp "$ROOT/data/icons/io.github.chrisby4563.Rustfin.svg" "$STAGE/share/icons/hicolor/scalable/apps/"
gtk4-update-icon-cache -q -f -t "$STAGE/share/icons/Adwaita" || true
gtk4-update-icon-cache -q -f -t "$STAGE/share/icons/hicolor" || true

# GTK / libadwaita translations
for lang in de en_GB; do
  mkdir -p "$STAGE/share/locale/$lang/LC_MESSAGES"
  for mo in gtk40 gtk40-properties libadwaita glib20; do
    [[ -f $MINGW/share/locale/$lang/LC_MESSAGES/$mo.mo ]] \
      && cp $MINGW/share/locale/$lang/LC_MESSAGES/$mo.mo "$STAGE/share/locale/$lang/LC_MESSAGES/"
  done
done

cp "$WIN/rustfin.ico" "$STAGE/"
cp "$ROOT/LICENSE" "$STAGE/LICENSE.txt"

echo "==> Building installer"
mkdir -p "$ROOT/_dist-win"
makensis -V2 -DVERSION="$VERSION" -DSTAGE="$(cygpath -w "$STAGE")" \
  -DOUTFILE="$(cygpath -w "$ROOT/_dist-win/Rustfin-$VERSION-setup.exe")" \
  "$(cygpath -w "$WIN/installer.nsi")"

# Portable ZIP as well
(cd "$ROOT/_dist-win" && rm -f "Rustfin-$VERSION-portable.zip" && zip -qr "Rustfin-$VERSION-portable.zip" Rustfin)

ls -la "$ROOT/_dist-win"
