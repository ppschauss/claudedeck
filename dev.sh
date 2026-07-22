#!/bin/bash
# Rust-Dev im Container — Unraid hat keine Toolchain (Konvention wie android-build).
# Aufruf: ./dev.sh cargo test   |   ./dev.sh cargo run --example spike -- …
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$DIR/.cargo-cache/registry" "$DIR/.cargo-cache/git" "$DIR/.cargo-cache/target"
[ -f "$DIR/secrets.env" ] && set -a && . "$DIR/secrets.env" && set +a
exec docker run --rm -i \
  -v "$DIR":/work -w /work \
  -v "$DIR/.cargo-cache/registry":/usr/local/cargo/registry \
  -v "$DIR/.cargo-cache/git":/usr/local/cargo/git \
  -e CARGO_TARGET_DIR=/work/.cargo-cache/target \
  -e "SPIKE_SSH_PASSWORD=${SPIKE_SSH_PASSWORD:-}" \
  rust:1-bookworm "$@"
