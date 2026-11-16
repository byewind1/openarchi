#!/bin/sh
# Keep cargo, rustc, rustdoc and formatter on one installed stable toolchain.
set -eu
if command -v rustup >/dev/null 2>&1; then
    RUSTC=$(rustup which --toolchain stable rustc)
    RUSTDOC=$(rustup which --toolchain stable rustdoc)
    export RUSTC RUSTDOC
    export PATH="$(dirname "$RUSTC"):$PATH"
    exec "$(rustup which --toolchain stable cargo)" "$@"
fi
exec cargo "$@"
