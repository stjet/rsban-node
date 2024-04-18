#pragma once

#include <nano/node/network.hpp>
#include <nano/node/repcrawler.hpp>

namespace nano
{
class election;
class node;
class node_config;
/** This class accepts elections that need further votes before they can be confirmed and bundles them in to single confirm_req packets */
class confirmation_solicitor final
{
public:
	confirmation_solicitor (nano::network &, nano::node_config const &);
	confirmation_solicitor (confirmation_solicitor const &) = delete;
	~confirmation_solicitor ();
	/** Prepare object for batching election confirmation requests*/
	void prepare (std::vector<nano::representative> const &);
	/** Broadcast the winner of an election if the broadcast limit has not been reached. Returns false if the broadcast was performed */
	bool broadcast (nano::election const &, nano::election_lock const & lock_a);
	/** Add an election that needs to be confirmed. Returns false if successfully added */
	bool add (nano::election const &, nano::election_lock const & lock_a);
	/** Dispatch bundled requests to each channel*/
	void flush ();
	rsnano::ConfirmationSolicitorHandle * handle;
};
}
