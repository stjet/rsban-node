#!/bin/bash

brew update
brew install coreutils
brew install rust
brew cask install xquartz
sudo util/build_prep/fetch_boost.sh
util/build_prep/macosx/build_qt.sh
git clone https://github.com/corrosion-rs/corrosion.git
cmake -Scorrosion -Bbuild -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
cmake --install build --config Release