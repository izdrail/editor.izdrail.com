FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive CARGO_HOME=/usr/local/cargo RUSTUP_HOME=/usr/local/rustup PATH=/usr/local/cargo/bin:$PATH
RUN apt-get update -qq && apt-get install -y -qq build-essential python3 git curl ca-certificates xz-utils clang pkg-config libxkbcommon-x11-dev libvulkan-dev libwayland-dev xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev libssl-dev cmake \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.98.0 --profile minimal
WORKDIR /src
