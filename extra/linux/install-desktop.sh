#!/bin/sh
# Register this extracted bundle with the current user's Linux desktop.
set -eu
bundle=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix=${1:-"$HOME/.local"}
case "$prefix" in /*) ;; *) printf '%s\n' 'Installation prefix must be absolute' >&2; exit 1;; esac
case "$bundle$prefix" in *'
'*) printf '%s\n' 'Newlines in installation paths are not supported' >&2; exit 1;; esac
[ -x "$bundle/gravity" ] || { printf '%s\n' 'Keep this script next to the gravity binary' >&2; exit 1; }
# Desktop Exec has its own escaping rules, independent of shell quoting.
exec_path=$(printf '%s' "$bundle/gravity" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g; s/%/%%/g')
mkdir -p "$prefix/share/applications" "$prefix/share/icons/hicolor/128x128/apps" "$prefix/share/icons/hicolor/scalable/apps"
install -m 644 "$bundle/window-icon.png" "$prefix/share/icons/hicolor/128x128/apps/dev.lapce.lapce.png"
install -m 644 "$bundle/logo.svg" "$prefix/share/icons/hicolor/scalable/apps/dev.lapce.lapce.svg"
{
  printf '%s\n' '[Desktop Entry]' 'Version=1.0' 'Type=Application' 'Name=Gravity Editor' 'Comment=Code editor' 'Categories=Development;IDE;' 'StartupWMClass=lapce' 'Icon=dev.lapce.lapce'
  printf 'Exec="%s" %%F\n' "$exec_path"
  printf '%s\n' 'Terminal=false' 'MimeType=text/plain;inode/directory;'
} > "$prefix/share/applications/dev.lapce.lapce.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$prefix/share/applications"; fi
if command -v gtk-update-icon-cache >/dev/null 2>&1 && [ -f "$prefix/share/icons/hicolor/index.theme" ]; then gtk-update-icon-cache -f -t "$prefix/share/icons/hicolor"; fi
printf 'Registered Gravity Editor for this bundle in %s/share/applications\n' "$prefix"
