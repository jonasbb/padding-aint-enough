#!/usr/bin/fish

set -l SCRIPT (realpath (dirname (status --current-filename))/traffic-logging/control-chrome.py)

for i in (seq $argv[1])
    set -l TMPDIR (mktemp --directory)
    pushd $TMPDIR

    python3 $SCRIPT $argv[2]
    popd
    mv $TMPDIR/website-log.json ./website-log-$i.json

    rm -rf $TMPDIR
end
