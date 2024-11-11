<p style="text-align:center;"><img src="/images/logo.svg" width"300px" height="auto" alt="Logo"></p>


[![Unit Tests](https://github.com/simpago/rsnano-node/actions/workflows/unit_tests.yml/badge.svg)](https://github.com/simpago/rsnano-node/actions/workflows/unit_tests.yml)
[![Discord](https://img.shields.io/badge/discord-join%20chat-orange.svg)](https://discord.gg/kBwvAyxEWE)


### What is RsNano?

RsNano is a Rust port of the original Nano node.

### Links & Resources

* [RsNano Website](https://rsnano.com)
* [Discord Chat](https://discord.gg/kBwvAyxEWE)
* [Twitter](https://twitter.com/gschauwecker)

### Installation

**Please mind that this project is still in its early stages and hasn't been thoroughly tested yet!**

## Option 1: Run the official docker image

    docker run -p 54000:54000 -v ~/NanoBeta:/root/NanoBeta simpago/rsnano:V1.0RC1 nano_node daemon --network=beta

## Option 2: Build your own docker image

    docker build -f docker/node/Dockerfile -t rsnano-node https://github.com/simpago/rsnano-node.git#develop

    docker run -p 54000:54000 -v ~/NanoBeta:/root/NanoBeta rsnano-node:latest node run --network=beta

## Option 3: Build from source

Currently you can only build RsNano on Linux and on Mac.

To just build and run the rsnano_node:

    git clone --recurse-submodules https://github.com/simpago/rsnano-node.git
    cd rsnano-node/rust/main
    cargo build --release
    cargo run --release -- node run

To install and run the rsnano_node executable:

    git clone --recurse-submodules https://github.com/simpago/rsnano-node.git
    cd rsnano-node
    cargo install --path rust/main
    rsnano_node node run

### Contact us

We want to hear about any trouble, success, delight, or pain you experience when
using RsNano. Let us know by [filing an issue](https://github.com/simpago/rsnano-node/issues), or joining us on [Discord](https://discord.gg/kBwvAyxEWE).
