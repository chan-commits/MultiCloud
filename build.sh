#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

case "${1:-}" in
  "")
    BUILD_PROFILE="debug"
    CARGO_BUILD_ARGS=()
    ;;
  --release)
    BUILD_PROFILE="release"
    CARGO_BUILD_ARGS=(--release)
    ;;
  *)
    printf 'usage: %s [--release]\n' "$(basename -- "$0")" >&2
    exit 2
    ;;
esac

for command_name in cargo rustc node npm; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'error: required command not found: %s\n' "$command_name" >&2
    exit 1
  fi
done

printf '%s\n' '==> Checking toolchain'
rustc --version
cargo --version
node --version
npm --version

printf '%s\n' '==> Fetching Rust dependencies'
cargo fetch

printf '%s\n' '==> Installing locked frontend dependencies'
npm --prefix frontend/web ci

printf '%s\n' '==> Checking Rust formatting'
cargo fmt --all -- --check

printf '%s\n' '==> Running Rust lint and tests'
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

printf '%s\n' '==> Checking and building frontend'
npm --prefix frontend/web run format:check
npm --prefix frontend/web run check
npm --prefix frontend/web run build

printf '==> Building the single multicloud %s binary\n' "$BUILD_PROFILE"
cargo build "${CARGO_BUILD_ARGS[@]}" -p multicloud

if [[ "$BUILD_PROFILE" == "release" ]]; then
  BINARY_PATH="$ROOT_DIR/target/release/multicloud"
else
  BINARY_PATH="$ROOT_DIR/target/debug/multicloud"
fi

printf '\nBuild complete:\n'
printf '  binary:   %s\n' "$BINARY_PATH"
printf '  frontend: %s\n' "$ROOT_DIR/frontend/web/dist"
