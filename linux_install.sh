#!/bin/bash
# bosectl Linux Installer
#
# pybmap uses only the Python standard library on Linux, so there is
# nothing to pip install: this checks the prerequisites and symlinks the
# repo's bosectl script onto PATH.

set -e
cd "$(dirname "$0")"

echo "=== bosectl Linux Installer ==="
echo ""

# 1. Python with Bluetooth socket support
# The first python3 on PATH may be a build without AF_BLUETOOTH (Homebrew/
# Linuxbrew, pyenv, conda). Fall back to the distribution's /usr/bin/python3,
# and pin whichever works in the launcher so bosectl doesn't depend on PATH.
# Override with PYTHON=/path/to/python3.
has_bluetooth() {
    "$1" -c 'import socket; socket.AF_BLUETOOTH; socket.BTPROTO_RFCOMM' 2>/dev/null
}

PY=""
for candidate in ${PYTHON:-} "$(command -v python3 || true)" /usr/bin/python3; do
    if [ -n "$candidate" ] && [ -x "$candidate" ] && has_bluetooth "$candidate"; then
        PY="$candidate"
        break
    fi
done
if [ -z "$PY" ]; then
    echo "No python3 with Bluetooth socket support (AF_BLUETOOTH) was found."
    echo "Install your distribution's python3 package, or set PYTHON=/path/to/python3."
    exit 1
fi
echo "Using Python: $PY"
echo ""

# 2. BlueZ, for pairing
if ! command -v bluetoothctl >/dev/null; then
    echo "Warning: bluetoothctl not found. Install BlueZ (package 'bluez') to pair headphones."
fi

# 3. Make bosectl script executable
echo "Configuring executable permissions..."
chmod +x bosectl

# 4. Install a launcher that runs bosectl with the Python found above
BIN_DIR="${BIN_DIR:-/usr/local/bin}"
LAUNCHER="$BIN_DIR/bosectl"
echo ""
echo "To make 'bosectl' accessible from anywhere on your system,"
echo "we will install a launcher at $LAUNCHER."
echo "Set BIN_DIR (e.g. BIN_DIR=\$HOME/.local/bin) to install elsewhere."
echo ""

if [ -d "$BIN_DIR" ] && [ -w "$BIN_DIR" ]; then
    SUDO=""
else
    SUDO="sudo"
    echo "$BIN_DIR is not writable; this requires administrator privileges."
fi

if printf '#!/bin/sh\nexec "%s" "%s" "$@"\n' "$PY" "$(pwd)/bosectl" \
        | $SUDO install -D -m 755 /dev/stdin "$LAUNCHER"; then
    echo ""
    echo "=== Installation Successful! ==="
    echo "Pair your headphones with bluetoothctl, then run 'bosectl status'."
else
    echo ""
    echo "=== Install Failed ==="
    echo "Could not install the launcher at $LAUNCHER."
    echo "You can still run it locally using: $PY $(pwd)/bosectl"
fi
