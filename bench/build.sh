#!/bin/sh
# Explicit owner-run build. Does not install toolchains or CUDA, or download models.
# Cargo may fetch pinned source dependencies. Device/runtime validation is separate.
set -eu
if [ "$#" -ne 2 ]; then
    printf '%s\n' 'usage: sh bench/build.sh candle|llamacpp|mistralrs cpu|cuda' >&2
    exit 2
fi
backend=$1
device=$2
case "$backend:$device" in
    candle:cpu|candle:cuda|llamacpp:cpu|llamacpp:cuda|mistralrs:cpu|mistralrs:cuda) ;;
    *) printf '%s\n' 'unsupported backend/device combination' >&2; exit 2 ;;
esac
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
command -v cargo
cargo --version
if [ "$device" = cuda ]; then
    command -v nvcc
    nvcc --version
    command -v nvidia-smi
    nvidia-smi
    printf '%s\n' 'CUDA build requested. Detection is not evidence of GPU inference.' >&2
fi
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
# Keep feature/backend artifacts separate, including when the owner has a
# CARGO_TARGET_DIR set. Otherwise a CPU build can overwrite a CUDA executable.
build_root=${BENCH_BUILD_ROOT:-"$root/target/bench-builds"}
case "$build_root" in
    /*) ;;
    *) build_root="$root/$build_root" ;;
esac
export CARGO_TARGET_DIR="$build_root/$backend-$device"
mkdir -p "$CARGO_TARGET_DIR"
if [ "$backend" = mistralrs ]; then
    if [ "$device" = cuda ]; then
        cargo build --locked --release --manifest-path inference/mistral-worker/Cargo.toml --features cuda
    else
        cargo build --locked --release --manifest-path inference/mistral-worker/Cargo.toml
    fi
    executable="$CARGO_TARGET_DIR/release/lapce-mistral-worker"
elif [ "$backend" = llamacpp ]; then
    if [ "$device" = cuda ]; then
        cargo build --locked --release -p lapce-ai --features llama-cpp-cuda
    else
        cargo build --locked --release -p lapce-ai --features llama-cpp
    fi
    executable="$CARGO_TARGET_DIR/release/lapce-ai"
else
    if [ "$device" = cuda ]; then
        cargo build --locked --release -p lapce-ai --features candle-cuda
    else
        cargo build --locked --release -p lapce-ai
    fi
    executable="$CARGO_TARGET_DIR/release/lapce-ai"
fi
if [ ! -x "$executable" ]; then
    printf '%s\n' 'Build did not produce the expected executable' >&2
    exit 1
fi
printf 'Worker: %s\n' "$executable"
printf '%s\n' 'Build finished. No model inference was performed. Save this build output.'
