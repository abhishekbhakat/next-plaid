#!/bin/bash
# Install script for semgrep

set -e

echo "Installing semgrep..."

# Detect OS
OS="$(uname -s)"

case "$OS" in
    Linux*|Darwin*)
        make install
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "Please build manually: cargo build --release"
        exit 1
        ;;
esac

echo "Installation complete!"
echo "Usage: semgrep --help"
