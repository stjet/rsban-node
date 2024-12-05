#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
cargo watch -s $SCRIPT_DIR/run_tests_lib.sh
