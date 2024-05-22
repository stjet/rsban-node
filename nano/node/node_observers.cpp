#include <nano/node/node_observers.hpp>

std::unique_ptr<nano::container_info_component> nano::collect_container_info (nano::node_observers & node_observers, std::string const & name)
{
	auto composite = std::make_unique<nano::container_info_composite> (name);
	composite->add_component (node_observers.blocks.collect_container_info ("blocks"));
	composite->add_component (node_observers.vote.collect_container_info ("vote"));
	composite->add_component (node_observers.account_balance.collect_container_info ("account_balance"));
	return composite;
}
