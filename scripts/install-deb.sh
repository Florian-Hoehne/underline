#!/usr/bin/env bash
# Installs the newest Underline deb package built by `cargo tauri build`.
# The elevation is asked for by pkexec, which shows GNOME's password dialog.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="$root/target/release/bundle/deb"

deb="$(ls -t "$dir"/*.deb 2>/dev/null | head -n 1 || true)"
if [[ -z "$deb" ]]; then
  echo "No deb package in $dir. Build it first: cd apps/underline-tauri && cargo tauri build" >&2
  exit 1
fi

echo "Installing $deb"
echo "Built $(date -r "$deb" '+%Y-%m-%d %H:%M')"
pkexec /usr/bin/apt-get install -y --reinstall "$deb"
echo "Installed: $(dpkg-query -W -f='${Package} ${Version}\n' underline 2>/dev/null || echo 'see apt output above')"
