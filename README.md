<p style="text-align:center;"><img src="/doc/images/logo.svg" width"300px" height="auto" alt="Logo"></p>


[![Unit Tests](https://github.com/simpago/rsnano-node/actions/workflows/unit_tests.yml/badge.svg)](https://github.com/simpago/rsnano-node/actions/workflows/unit_tests.yml)
[![Discord](https://img.shields.io/badge/discord-join%20chat-orange.svg)](https://discord.gg/kBwvAyxEWE)


### What is RsBanano?

RsBanano is a fork of RsNano, which is a Rust port of the original Nano node.

### Links & Resources

* [RsNano Website](https://rsnano.com)
* [Discord Chat](https://discord.gg/kBwvAyxEWE)
* [Twitter](https://twitter.com/gschauwecker)

### Installation

**Please mind that this project is still in its early stages and hasn't been thoroughly tested yet!**

## Option 1: Run the official Docker image

    TBA

## Option 2: Build your own Docker image

    docker build -f scripts/docker/node/Dockerfile -t rsbanano-node https://github.com/stjet/rsbanano-node.git#develop

    docker run -p 54019:54019 -v ~/BananoBeta:/root/BananoBeta rsbanano-node:latest node run --network=beta

## Option 3: Build from source

Currently you can only build RsBanano on Linux and on Mac.

To just build and run the rsbanano_node:

    git clone https://github.com/stjet/rsbanano-node.git
    cd rsbanano-node/main
    cargo build --release
    cargo run --release -- node run --network=beta

To install and run the rsbanano_node executable:

    git clone https://github.com/stjet/rsbanano-node.git
    cd rsbanano-node
    cargo install --path main
    rsbanano_node node run --network=beta

### Contact us

We want to hear about any trouble, success, delight, or pain you experience when
using RsBanano. Let us know by [filing an issue](https://github.com/stjet/rsbanano-node/issues), or joining the [RsNano Discord](https://discord.gg/kBwvAyxEWE) for issues common to both RsNano and RsBanano.

# The codebase

The Rust code is structured according to A-frame architecture and is built with nullable infrastructure. This design and testing approach is extensively documented here:

[http://www.jamesshore.com/v2/projects/nullables/testing-without-mocks]

The following diagram shows how the crates are organized. The crates will be split up more when the codebase grows.

![crate diagram](http://www.plantuml.com/plantuml/proxy?cache=no&fmt=svg&src=https://raw.github.com/rsnano-node/rsnano-node/develop/doc/crates.puml)

* `main`: The node executable.
* `daemon`: Starts the node and optionally the RPC server.
* `node`:The node implementation.
* `rpc_server`: Implemenation of the RPC server.
* `ledger`: Ledger implementation. It is responsible for the consinstency of the data stores.
* `store_lmdb`: LMDB implementation of the data stores.
* `messages`: Message types that nodes use for communication.
* `network`: Manage outbound/inbound TCP channels to/from other nodes.
* `core`: Contains the basic types like `BlockHash`, `Account`, `KeyPair`,...
* `nullables`: Nullable wrappers for infrastructure libraries.

