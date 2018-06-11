#!/usr/bin/fish

# This file is called from control-chrome directly before the website is loaded

pushd (dirname (status --current-filename))

./flush-unbound.fish
./prefetch-unbound.fish

echo "Before Experiment"
dig @127.0.0.1 start.example. >/dev/null 2>&1
echo
