#!/usr/bin/fish

# This file is called from control-chrome directly before the website is loaded

pushd (dirname (status --current-filename))

echo Flush
./flush-unbound.fish
echo Prefetch
./prefetch-unbound.fish

echo "start.example marker query"
dig @127.0.0.1 start.example. >/dev/null 2>&1
echo
