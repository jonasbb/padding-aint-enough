#!/usr/bin/fish
pushd (dirname (status --current-filename))
set -l PREFETCH_DOMAINS ./alexa-top30k-eff-tlds.txt
set -l LINES (wc -l "$PREFETCH_DOMAINS" | cut -d ' ' -f 1)
set -l BATCHSIZE 50

for range in (seq 1 $BATCHSIZE $LINES)
    # prefetch 100 entries at a time
    ../dns-par --servers "127.0.0.1" --file (tail -n+"$range" "$PREFETCH_DOMAINS" | head -n"$BATCHSIZE" | psub) >/dev/null
    sleep .5
end
sleep 2
