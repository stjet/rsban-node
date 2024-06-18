#pragma once

namespace nano
{
enum class election_behavior
{
	manual,
	priority,
	/**
	 * Hinted elections:
	 * - shorter timespan
	 * - limited space inside AEC
	 */
	hinted,
	/**
	 * Optimistic elections:
	 * - shorter timespan
	 * - limited space inside AEC
	 * - more frequent confirmation requests
	 */
	optimistic,
};
}
