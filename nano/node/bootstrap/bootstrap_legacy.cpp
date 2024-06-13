#include "nano/lib/rsnano.hpp"

#include <nano/node/bootstrap/bootstrap_bulk_push.hpp>
#include <nano/node/bootstrap/bootstrap_frontier.hpp>
#include <nano/node/bootstrap/bootstrap_legacy.hpp>
#include <nano/node/node.hpp>

#include <boost/format.hpp>

nano::bootstrap_attempt_legacy::bootstrap_attempt_legacy (rsnano::BootstrapAttemptHandle * handle) :
	nano::bootstrap_attempt (handle)
{
}

void nano::bootstrap_attempt_legacy::get_information (boost::property_tree::ptree & tree_a)
{
	rsnano::rsn_bootstrap_attempt_legacy_get_information (handle, (void *)&tree_a);
}
