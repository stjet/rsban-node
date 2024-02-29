#include "nano/lib/rsnano.hpp"

#include <nano/node/bootstrap/bootstrap_attempt.hpp>
#include <nano/node/bootstrap/bootstrap_bulk_push.hpp>
#include <nano/node/bootstrap/bootstrap_legacy.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>

#include <boost/format.hpp>

nano::bulk_push_client::bulk_push_client (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::bootstrap_client> const & connection_a, std::shared_ptr<nano::bootstrap_attempt_legacy> const & attempt_a) :
	handle {rsnano::rsn_bulk_push_client_create(connection_a->handle, node_a->ledger.handle, attempt_a->handle)}
{
}

nano::bulk_push_client::~bulk_push_client ()
{
	rsnano::rsn_bulk_push_client_destroy(handle);
}

void nano::bulk_push_client::start ()
{
	return rsnano::rsn_bulk_push_client_start(handle);
}

bool nano::bulk_push_client::get_result(){
	return rsnano::rsn_bulk_push_client_get_result(handle);
}

void nano::bulk_push_client::set_result(bool value)
{
	rsnano::rsn_bulk_push_client_set_result(handle, value);
}

