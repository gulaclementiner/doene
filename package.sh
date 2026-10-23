#!/usr/bin/env bash
# Build a portable Windows package (no Rust needed to run it): dist/Doene-windows-x64.zip
# Needs: rustup target x86_64-pc-windows-gnu, mingw-w64, zip.
set -euo pipefail
cd "$(dirname "$0")"
TARGET=x86_64-pc-windows-gnu
OUT=${CARGO_TARGET_DIR:-target}
cargo build --release --target "$TARGET"

rm -rf dist/Doene && mkdir -p dist/Doene
cp "$OUT/$TARGET/release/doene.exe" dist/Doene/Doene.exe
cat > dist/Doene/README.txt <<'TXT'
Doene (Döne) - portable build
=============================
Double-click Doene.exe. No installation and no Rust needed.

Data is stored in %APPDATA%\Döne (file done.db).
To keep data next to the app instead, run from cmd:
    set DONE_DATA_DIR=%CD%\data
    Doene.exe
A console window opens alongside the app; it shows error messages.
TXT
rm -f dist/Doene-windows-x64.zip
(cd dist && zip -qr Doene-windows-x64.zip Doene)
ls -lh dist/Doene-windows-x64.zip
