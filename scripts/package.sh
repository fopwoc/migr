#!/usr/bin/env bash
# Builds the Apple Silicon binary and packs it as migr-<version>-macos.tar.gz in the given folder.
set -euo pipefail

out="${1:?usage: package.sh <output dir>}"
target=aarch64-apple-darwin

rustup target add "$target" >/dev/null
cargo build --release --locked --target "$target"
binary="target/$target/release/migr"

version="$("$binary" --version | awk '{print $2}')"
mkdir -p "$out"
archive="$out/migr-$version-macos.tar.gz"
tar -czf "$archive" -C "$(dirname "$binary")" migr
echo "$archive"
