#!/bin/bash
set -euox pipefail

COMPILER=${COMPILER:-gcc}

echo "Compiler: '${COMPILER}'"

# Common dependencies needed for building & testing
DEBIAN_FRONTEND=noninteractive apt-get update -qq

DEBIAN_FRONTEND=noninteractive apt-get install -yqq \
build-essential \
g++ \
curl \
wget \
curl \
python3 \
zlib1g-dev \
cmake \
git \
valgrind \
libssl-dev \
pkg-config

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- -y

pushd ..
mkdir corrosion
pushd corrosion
git clone https://github.com/AndrewGaspar/corrosion.git \
    && cmake -Scorrosion -Bbuild -DCMAKE_BUILD_TYPE=Release \
    && cmake --build build --config Release \
    && cmake --install build --config Release 
popd
popd

# Compiler specific setup
$(dirname "$BASH_SOURCE")/prepare-${COMPILER}.sh
