#!/bin/sh
# Builds the release binary and packs it into dist/the_garden-linux-x86_64.tar.gz
set -eu
cd "$(dirname "$0")/.."
cargo build --release
rm -rf dist/the_garden
mkdir -p dist/the_garden
cp target/release/the_garden dist/the_garden/
cp README.md dist/the_garden/README.md
cp assets/fonts/OFL-Nunito.txt dist/the_garden/LICENSE-font.txt
cat > dist/the_garden/the_garden.desktop <<'DESK'
[Desktop Entry]
Type=Application
Name=The Garden (beta)
Comment=Cozy co-op farming on a shared island
Exec=the_garden
Terminal=false
Categories=Game;
DESK
tar -C dist -czf dist/the_garden-linux-x86_64.tar.gz the_garden
ls -la dist/the_garden-linux-x86_64.tar.gz
