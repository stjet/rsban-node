# Rust codebase

This folder contains all the Rust code of RsNano. The following diagram shows how the crates are organized. The crates will be split up more when the codebase grows.

![cached image](http://www.plantuml.com/plantuml/proxy?src=https://raw.github.com/simpago/rsnano-node/develop/rust/doc/crates.puml)

* `ffi`: Contains all the glue code to connect the C++ and the Rust part
* `node`: Contains the node implementation
* `ledger`: Contains the ledger implementation
* `store_traits`: Contains traits for the data stores. These traits have to be implemented if you want to add a new type of data store.
* `core`: Contains the basic types like `BlockHash`, `Account`, `KeyPair`,...
* `store_lmdb`: Contains the LMDB data store implementation
* `store_mem`: Contains the in-memory data store implementation which is used by unit tests