#!/usr/bin/fish
set -l RECORD_SCRIPT (realpath (dirname (status --current-filename))/record-websites.fish)

for DOM in (cat $argv[1])
    mkdir -p $DOM
    pushd $DOM
    fish $RECORD_SCRIPT 10 http://$DOM/
    popd
end
