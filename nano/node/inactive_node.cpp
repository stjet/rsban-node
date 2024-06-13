#include <nano/node/active_elections.hpp>
#include <nano/node/inactive_node.hpp>
#include <nano/node/node.hpp>

nano::inactive_node::inactive_node (std::filesystem::path const & path_a, std::filesystem::path const & config_path_a, nano::node_flags & node_flags_a) :
	node_wrapper (path_a, config_path_a, node_flags_a),
	node (node_wrapper.node)
{
	node_wrapper.node->active.stop ();
}

nano::inactive_node::inactive_node (std::filesystem::path const & path_a, nano::node_flags & node_flags_a) :
	inactive_node (path_a, path_a, node_flags_a)
{
}

nano::node_flags const & nano::inactive_node_flag_defaults ()
{
	static nano::node_flags node_flags;
	node_flags.set_inactive_node (true);
	node_flags.set_read_only (true);
	auto gen_cache = node_flags.generate_cache ();
	gen_cache.enable_reps (false);
	gen_cache.enable_cemented_count (false);
	gen_cache.enable_unchecked_count (false);
	gen_cache.enable_account_count (false);
	node_flags.set_generate_cache (gen_cache);
	node_flags.set_disable_bootstrap_listener (true);
	node_flags.set_disable_tcp_realtime (true);
	return node_flags;
}
