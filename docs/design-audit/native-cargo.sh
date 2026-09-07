#!/usr/bin/env sh
# Reproduce native macOS checks using the repository-local audit toolchain.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
export CARGO_HOME="$root/docs/design-audit/tmp/native-rust/cargo-home"
export RUSTUP_HOME="$root/docs/design-audit/tmp/native-rust/rustup-home"
export CARGO_TARGET_DIR="$root/target/native-macos"
export TMPDIR="$root/docs/design-audit/tmp"
export PATH="$CARGO_HOME/bin:$PATH"
exec "$CARGO_HOME/bin/cargo" "$@"
