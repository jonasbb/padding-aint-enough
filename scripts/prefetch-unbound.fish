#!/usr/bin/fish
pushd (dirname (status --current-filename))
../dns-par --servers "127.0.0.1" --file ./alexa-top1000-tlds.txt >/dev/null
sleep 2
