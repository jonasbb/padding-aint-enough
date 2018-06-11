#!/usr/bin/fish
pushd (dirname (status --current-filename))
../dns-par --server "127.0.0.1" --file ./alexa-top1000-tlds.txt
