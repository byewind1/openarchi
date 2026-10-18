#!/bin/sh
# Prefer rustup's installed stable toolchain over older Homebrew binaries.
set -eu
if [ -x "$HOME/.cargo/bin/rustup" ]; then
  OPENARCHI_RUSTC=$("$HOME/.cargo/bin/rustup" which --toolchain stable rustc)
  OPENARCHI_CARGO=$("$HOME/.cargo/bin/rustup" which --toolchain stable cargo)
  export RUSTC="$OPENARCHI_RUSTC"
  export RUSTDOC="$(dirname "$OPENARCHI_RUSTC")/rustdoc"
  # cargo resolves cargo-clippy/rustfmt through PATH; keep it on the same stable toolchain.
  export PATH="$(dirname "$OPENARCHI_RUSTC"):$PATH"
  exec "$OPENARCHI_CARGO" "$@"
fi
exec cargo "$@"
