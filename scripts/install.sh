#!/usr/bin/env sh
# Install one checksum-verified native release in a user-owned directory.
set -eu

version=latest
install_dir=${THEYWORK_INSTALL_DIR:-}
base_url=https://github.com/Giuseppecarte/they-work/releases
usage() {
    echo "Usage: sh scripts/install.sh [--version vX.Y.Z] [--install-dir PATH]"
    echo "Downloads a native release, verifies SHA256, and installs without sudo."
}
while [ "$#" -gt 0 ]; do
    case "$1" in
        --version|--install-dir)
            [ "$#" -ge 2 ] || { usage >&2; exit 2; }
            case "$1" in --version) version=$2 ;; --install-dir) install_dir=$2 ;; esac
            shift 2 ;;
        --help|-h) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
done
if [ -z "$install_dir" ]; then
    install_dir="${HOME:?Set HOME or pass --install-dir}/.local/bin"
fi
if [ -d "$install_dir/they-work" ]; then
    echo "$install_dir/they-work is a directory; choose another --install-dir. Nothing installed." >&2
    exit 1
fi
case "$version" in latest|v[0-9]*.[0-9]*.[0-9]*) ;; *) echo "Invalid release version: $version" >&2; exit 2 ;; esac
case "$version" in *[!a-zA-Z0-9.-]*) echo "Invalid release version" >&2; exit 2 ;; esac
case "$(uname -s)" in
    Darwin) system=apple-darwin ;;
    Linux) system=unknown-linux-musl ;;
    *) echo "Use scripts/install.ps1 on Windows, or build from source; see INSTALL.md." >&2; exit 1 ;;
esac
case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=aarch64 ;;
    *) echo "No native release for $(uname -m); build from source (INSTALL.md)." >&2; exit 1 ;;
esac
for tool in curl tar mktemp awk; do
    command -v "$tool" >/dev/null 2>&1 || { echo "Required command missing: $tool" >&2; exit 1; }
done
if command -v sha256sum >/dev/null 2>&1; then
    checksum() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then
    checksum() { shasum -a 256 "$1" | awk '{print $1}'; }
else
    echo "SHA256 verification requires sha256sum or shasum." >&2; exit 1
fi
asset="they-work-$arch-$system.tar.gz"
if [ "$version" = latest ]; then url="$base_url/latest/download"; else url="$base_url/download/$version"; fi
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/they-work-install.XXXXXXXX")
staged=
trap 'rm -rf "$work_dir"; if [ -n "$staged" ]; then rm -f "$staged"; fi' EXIT HUP INT TERM
echo "Downloading $asset ($version) ..."
if ! curl --fail --show-error --silent --location --proto '=https' --tlsv1.2 "$url/$asset" -o "$work_dir/$asset"; then
    echo "No downloadable native asset for $version/$arch-$system. Nothing installed. The older v0.1.0 release is Docker-only; see INSTALL.md to build this checkout." >&2
    exit 1
fi
curl --fail --show-error --silent --location --proto '=https' --tlsv1.2 "$url/SHA256SUMS" -o "$work_dir/SHA256SUMS"
expected=$(awk -v name="$asset" '$2 == name { print $1 }' "$work_dir/SHA256SUMS")
if [ "${#expected}" -ne 64 ] || [ "$expected" != "$(checksum "$work_dir/$asset")" ]; then
    echo "SHA256 verification failed; nothing installed." >&2; exit 1
fi
# Extract only the expected member; no archive paths are trusted.
tar -xzf "$work_dir/$asset" -C "$work_dir" they-work
if [ ! -f "$work_dir/they-work" ] || [ -L "$work_dir/they-work" ]; then
    echo "Release archive has no regular they-work binary; nothing installed." >&2; exit 1
fi
mkdir -p "$install_dir"
staged=$(mktemp "$install_dir/.they-work.XXXXXXXX")
cp "$work_dir/they-work" "$staged"
chmod 755 "$staged"
mv -f "$staged" "$install_dir/they-work"
staged=
echo "Installed: $install_dir/they-work"
case ":$PATH:" in
    *":$install_dir:"*) echo "Start with: they-work --setup (your sources) or they-work --demo" ;;
    *) echo "Start with: \"$install_dir/they-work\" --setup"
       echo "Add $install_dir to your shell PATH to use the they-work command." ;;
esac
