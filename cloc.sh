#!/bin/sh
cloc --include-lang=Rust,C,C++,"C/C++ Header" --exclude-list-file=cloc-excludes.txt nano rust

