#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/daemonconfig.hpp>
#include <nano/node/node.hpp>
#include <nano/node/node_wrapper.hpp>

nano::node_wrapper::node_wrapper (std::filesystem::path const & path_a, std::filesystem::path const & config_path_a, nano::node_flags & node_flags_a) :
	network_params{ nano::network_constants::active_network () },
	async_rt (std::make_shared<rsnano::async_runtime> (true)),
	work{ network_params.network, 1 }
{
	/*
	 * @warning May throw a filesystem exception
	 */
	std::filesystem::create_directories (path_a);

	boost::system::error_code error_chmod;
	nano::set_secure_perm_directory (path_a, error_chmod);

	nano::daemon_config daemon_config{ path_a, network_params };
	auto tmp_overrides{ node_flags_a.config_overrides () };
	node_flags_a.set_config_overrides (tmp_overrides);

	auto & node_config = daemon_config.node;
	node_config.peering_port = 24000;

	node = std::make_shared<nano::node> (*async_rt, path_a, node_config, work, node_flags_a);
}

nano::node_wrapper::~node_wrapper ()
{
	node->stop ();
}
