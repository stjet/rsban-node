# Block Cementation


![uml diagram](http://www.plantuml.com/plantuml/proxy?cache=no&fmt=svg&src=https://raw.github.com/simpago/rsnano-node/develop/rust/doc/cementation.puml)

This module is responsible for cementing blocks. 'Cementing' means to mark a block as confirmed in the database.

If we cement a block we have to make sure that all of its dependencies get cemented too. For example there could be an uncemented previous block, or there could be an uncemented send block if whe try to cement a receive block. So we have to make sure that all of those blocks get cemented too in the correct order.