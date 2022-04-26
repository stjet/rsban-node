<p style="text-align:center;"><img src="/images/logo.svg" width"300px" height="auto" alt="Logo"></p>



[![Tests](https://github.com/simpago/rsnano-node/workflows/Tests/badge.svg)](https://github.com/simpago/rsnano-node/actions?query=workflow%3ATests)
[![Discord](https://img.shields.io/badge/discord-join%20chat-orange.svg)](https://discord.gg/kBwvAyxEWE)


### What is RsNano?

RsNano is a Rust port of the original Nano node.

### Links & Resources

* [RsNano Website](https://rsnano.com)
* [Discord Chat](https://discord.gg/kBwvAyxEWE)
* [Twitter](https://twitter.com/gschauwecker)

### Installation

**Please mind that this project is still in its early stages and hasn't been thoroughly tested yet!**

Currently you can only build RsNano on Linux by yourself.

Install the cmake plugin [Corrosion](https://github.com/corrosion-rs/corrosion) for building Rust projects with cmake:

    git clone https://github.com/AndrewGaspar/corrosion.git
    # Optionally, specify -DCMAKE_INSTALL_PREFIX=<target-install-path>. You can install Corrosion anyway
    cmake -Scorrosion -Bbuild -DCMAKE_BUILD_TYPE=Release
    cmake --build build --config Release
    # This next step may require sudo or admin privileges if you're installing to a system location,
    # which is the default.
    cmake --install build --config Release

Build the nano-node. The official [nano-node build instructions](https://docs.nano.org/integration-guides/build-options/) still apply for RsNano.

    git clone https://github.com/simpago/rsnano-node.git
    cd rsnano-node
    export BOOST_ROOT=`pwd`/../boost_build
    bash util/build_prep/bootstrap_boost.sh -m -B 1.73

    cmake -G "Unix Makefiles" -DNANO_TEST=ON -DACTIVE_NETWORK=nano_dev_network -DNANO_WARN_TO_ERR=ON -DCMAKE_BUILD_TYPE=Debug -DCMAKE_VERBOSE_MAKEFILE=ON -DNANO_STACKTRACE_BACKTRACE=ON .

    make nano_node
    cp nano_node ../nano_node && cd .. && ./nano_node --diagnostics

### Contact us

We want to hear about any trouble, success, delight, or pain you experience when
using RsNano. Let us know by [filing an issue](https://github.com/simpago/rsnano-node/issues), or joining us on [Discord](https://discord.gg/kBwvAyxEWE).
