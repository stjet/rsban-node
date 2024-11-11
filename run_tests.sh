#!/bin/bash
if build/core_test && cargo test --manifest-path=rust/Cargo.toml
then
	notify-send -i face-smile "Tests Passed!"
	play -q sounds/success.ogg
else 
	notify-send -i face-worried "Tests Failed!"
	play -q sounds/failed.ogg
fi
