#!/bin/bash
set -euox pipefail

#Homebrew randomly fails to update. Retry 5 times with 15s interval
for i in {1..5}; do brew update && break || { echo "Update failed, retrying..."; sleep 15; }; done

brew install coreutils

pushd ..
git clone https://github.com/corrosion-rs/corrosion.git
cmake -Scorrosion -Bcorrosion_build -DCMAKE_BUILD_TYPE=Release
cmake --build corrosion_build --config Release
sudo cmake --install corrosion_build --config Release
popd