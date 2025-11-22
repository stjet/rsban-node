#!/bin/sh
set -euox pipefail

COMPILER=${COMPILER:-gcc}
echo "Compiler: '${COMPILER}'"

apk update
apk add --no-cache \
    build-base \
    curl \
    wget \
    python3 \
    git \
    openssl-dev \
    bash \
    musl-dev \
    cmake \
    linux-headers \
    pkgconfig

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
