#pragma once

#include <nano/node/bootstrap/bootstrap_attempt.hpp>

#include <boost/property_tree/ptree_fwd.hpp>

namespace nano
{
/**
 * Legacy bootstrap session. This is made up of 3 phases: frontier requests, bootstrap pulls, bootstrap pushes.
 */
class bootstrap_attempt_legacy : public bootstrap_attempt
{
public:
	explicit bootstrap_attempt_legacy (rsnano::BootstrapAttemptHandle * handle);
};
}
