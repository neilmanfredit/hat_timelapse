#!/bin/sh
# Install the latest Hat Timelapse release on macOS or Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/neilmanfredit/hat_timelapse/main/install.sh | sh
#
# The command-line engine goes in ~/bin (set HAT_TIMELAPSE_DIR to choose another
# folder). On macOS the app goes in ~/Applications; on Linux it is added to the
# applications menu.
set -eu

REPO="neilmanfredit/hat_timelapse"
DEST="${HAT_TIMELAPSE_DIR:-$HOME/bin}"

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Darwin) target="macos-universal" ;;
  Linux)
    case "$arch" in
      x86_64 | amd64) target="linux-x86_64" ;;
      aarch64 | arm64) target="linux-aarch64" ;;
      *) echo "Sorry, there is no Linux build for $arch yet. See docs/INSTALL.md to build from source." >&2; exit 1 ;;
    esac
    ;;
  *) echo "This installer is for macOS and Linux. For Windows, see docs/INSTALL.md." >&2; exit 1 ;;
esac

echo "Finding the latest Hat Timelapse release…"
url=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
  grep -o "https://[^\"]*/hat_timelapse-[^\"]*-$target\.tar\.gz" | head -n 1)
if [ -z "$url" ]; then
  echo "No $target download was found in the latest release." >&2
  exit 1
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "Downloading $url"
curl -fsSL "$url" -o "$tmp/hat_timelapse.tar.gz"
tar -xzf "$tmp/hat_timelapse.tar.gz" -C "$tmp"
bin=$(find "$tmp" -type f -name hat_timelapse -not -path "*.app/*" | head -n 1)
[ -n "$bin" ] || bin=$(find "$tmp" -type f -name hat_timelapse | head -n 1)
if [ -z "$bin" ]; then
  echo "The download did not contain the hat_timelapse program." >&2
  exit 1
fi

mkdir -p "$DEST"
cp "$bin" "$DEST/hat_timelapse"
chmod 755 "$DEST/hat_timelapse"
if [ "$os" = "Darwin" ]; then
  xattr -d com.apple.quarantine "$DEST/hat_timelapse" 2>/dev/null || true
fi

echo
echo "Installed: $DEST/hat_timelapse"
"$DEST/hat_timelapse" --version

# The desktop app.
if [ "$os" = "Darwin" ]; then
  app=$(find "$tmp" -maxdepth 3 -type d -name "Hat Timelapse.app" | head -n 1)
  if [ -n "$app" ]; then
    apps="$HOME/Applications"
    mkdir -p "$apps"
    rm -rf "$apps/Hat Timelapse.app"
    cp -R "$app" "$apps/"
    xattr -dr com.apple.quarantine "$apps/Hat Timelapse.app" 2>/dev/null || true
    echo "Installed: $apps/Hat Timelapse.app  (open it from Finder, Launchpad or Spotlight)"
  fi
else
  gui=$(find "$tmp" -type f -name hat_timelapse_app | head -n 1)
  if [ -n "$gui" ]; then
    cp "$gui" "$DEST/hat_timelapse_app"
    chmod 755 "$DEST/hat_timelapse_app"
    share="${XDG_DATA_HOME:-$HOME/.local/share}"
    mkdir -p "$share/applications" "$share/icons/hicolor/256x256/apps"
    icon=$(find "$tmp" -type f -name hat-timelapse.png | head -n 1)
    [ -n "$icon" ] && cp "$icon" "$share/icons/hicolor/256x256/apps/hat-timelapse.png"
    desktop=$(find "$tmp" -type f -name hat-timelapse.desktop | head -n 1)
    if [ -n "$desktop" ]; then
      sed "s|@BIN@|$DEST/hat_timelapse_app|" "$desktop" > "$share/applications/hat-timelapse.desktop"
    fi
    echo "Installed: $DEST/hat_timelapse_app  (Hat Timelapse in your applications menu)"
  fi
fi

case ":$PATH:" in
  *":$DEST:"*) ;;
  *)
    shell_rc="$HOME/.profile"
    case "${SHELL:-}" in
      */zsh) shell_rc="$HOME/.zshrc" ;;
      */bash) shell_rc="$HOME/.bashrc" ;;
    esac
    echo
    echo "$DEST is not on your PATH yet. Run this once, then open a new Terminal window:"
    echo "  echo 'export PATH=\"$DEST:\$PATH\"' >> $shell_rc"
    ;;
esac

echo
echo "Next: run 'hat_timelapse doctor' to check LightCraft, ffmpeg and exiftool."
