#include <nano/lib/blocks.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/bootstrap/bootstrap_server.hpp>
#include <nano/node/transport/transport.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/account.hpp>
#include <nano/store/block.hpp>
#include <nano/store/component.hpp>
#include <nano/store/confirmation_height.hpp>

/*
 * bootstrap_server_config
 */

nano::error nano::bootstrap_server_config::deserialize (nano::tomlconfig & toml)
{
	toml.get ("max_queue", max_queue);
	toml.get ("threads", threads);
	toml.get ("batch_size", batch_size);

	return toml.get_error ();
}

void nano::bootstrap_server_config::load_dto (rsnano::BootstrapServerConfigDto const & dto)
{
	max_queue = dto.max_queue;
	threads = dto.threads;
	batch_size = dto.batch_size;
}

rsnano::BootstrapServerConfigDto nano::bootstrap_server_config::to_dto () const
{
	return { max_queue, threads, batch_size };
}
