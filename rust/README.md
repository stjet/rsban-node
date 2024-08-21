# Rust codebase

This folder contains all the Rust code of RsNano. 

The Rust code is structured according to A-frame architecture and is built with nullable infrastructure. This design and testing approach is extensively documented here:

[http://www.jamesshore.com/v2/projects/nullables/testing-without-mocks]

The following diagram shows how the crates are organized. The crates will be split up more when the codebase grows.

![crate diagram](http://www.plantuml.com/plantuml/proxy?cache=no&fmt=svg&src=https://raw.github.com/simpago/rsnano-node/develop/rust/doc/crates.puml)

* `ffi`: Contains all the glue code to connect the C++ and the Rust part
* `main`: Contains the pure Rust node executable
* `node`: Contains the node implementation
* `ledger`: Contains the ledger implementation with LMDB. It is responsible for the consinstency of the data stores.
* `messages`: Message types that nodes use for communication
* `core`: Contains the basic types like `BlockHash`, `Account`, `KeyPair`,...
* `nullables`: Nullable wrappers for infrastructure libraries