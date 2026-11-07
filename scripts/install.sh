#!/bin/sh
# Install the `cantin` command-line tool (macOS / Linux). No Rust needed.
#   curl -fsSL https://raw.githubusercontent.com/Darshan-Subramanya/cantin/main/scripts/install.sh | sh
# Env: CANTIN_INSTALL_DIR (default ~/.local/bin, or /usr/local/bin if writable)
#      CANTIN_BASE_URL   (default: latest GitHub release)
set -eu

BASE_URL="${CANTIN_BASE_URL:-https://github.com/Darshan-Subramanya/cantin/releases/latest/download}"

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Darwin) os_part=apple-darwin ;;
  Linux) os_part=unknown-linux-gnu ;;
  *) echo "cantin: unsupported OS '$os' (use the Windows installer script on Windows)" >&2; exit 1 ;;
esac
case "$arch" in
  arm64 | aarch64) arch_part=aarch64 ;;
  x86_64 | amd64) arch_part=x86_64 ;;
  *) echo "cantin: unsupported CPU '$arch'" >&2; exit 1 ;;
esac
if [ "$os_part" = unknown-linux-gnu ] && [ "$arch_part" = aarch64 ]; then
  echo "cantin: Linux ARM builds are not published yet; build from source with cargo" >&2
  exit 1
fi
target="$arch_part-$os_part"

if [ -n "${CANTIN_INSTALL_DIR:-}" ]; then
  dir=$CANTIN_INSTALL_DIR
elif [ -w /usr/local/bin ]; then
  dir=/usr/local/bin
else
  dir="$HOME/.local/bin"
fi
mkdir -p "$dir"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM
url="$BASE_URL/cantin-$target.tar.gz"
echo "Downloading $url"
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$url" -o "$tmp/cantin.tar.gz"
else
  wget -qO "$tmp/cantin.tar.gz" "$url"
fi
tar -xzf "$tmp/cantin.tar.gz" -C "$tmp"
install -m 755 "$tmp/cantin" "$dir/cantin"
if [ "$os" = Darwin ]; then
  xattr -d com.apple.quarantine "$dir/cantin" 2>/dev/null || true
fi

echo "Installed cantin to $dir/cantin"
case ":$PATH:" in
  *":$dir:"*) echo "Try: cantin scan ~" ;;
  *)
    echo
    echo "$dir is not on your PATH. Add it with:"
    echo "  echo 'export PATH=\"$dir:\$PATH\"' >> ~/.zshrc   # or ~/.bashrc"
    echo "then open a new terminal and run: cantin scan ~"
    ;;
esac
