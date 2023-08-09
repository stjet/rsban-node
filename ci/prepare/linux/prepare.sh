#!/bin/bash
set -euox pipefail

COMPILER=${COMPILER:-gcc}

echo "Compiler: '${COMPILER}'"

# Common dependencies needed for building & testing
apt-get update -qq

DEBIAN_FRONTEND=noninteractive apt-get install -yqq \
build-essential \
g++ \
wget \
python3 \
zlib1g-dev \
cmake \
git \
valgrind 

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