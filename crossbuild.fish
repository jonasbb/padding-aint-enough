#!/usr/bin/env fish

set -l CONST_TO_REPLACE "SIZE_INSERT_COST"
set -l CONST_RANGE (seq 10 30)

function print_id
    string join "-" (rg --only-matching --replace '$1' '(\\d+);' ./sequences/src/constants.rs)
end

function replace_const --description "replace_const <const_name> <const_value>"
    set -l CONST_NAME $argv[1]
    set -l CONST_VALUE $argv[2]

    sed -i 's/^\\(.*\\)'$CONST_NAME'\\(.*= \\)\\(.*\\);/\\1'$CONST_NAME'\\2'$CONST_VALUE';/g' ./sequences/src/constants.rs
end

for val in $CONST_RANGE
    replace_const $CONST_TO_REPLACE $val
    set -l ID (print_id)
    echo $ID
    cross build --release --target=x86_64-unknown-linux-musl --package dns-sequence
    cp ./target/x86_64-unknown-linux-musl/release/dns-sequence ./dns-sequence-$ID
end
