#!/bin/sh
rust_code=`cloc --include-lang=Rust --exclude-dir=CMakeFiles --json rust | jq '.Rust.code'`
sum_code=`cloc --include-lang=Rust,C,C++,"C/C++ Header" --exclude-dir=CMakeFiles --json nano rust | jq '.SUM.code'`
perc=`echo "$rust_code/$sum_code*100" |bc -l`
echo $perc

