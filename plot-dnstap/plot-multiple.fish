#!/usr/bin/env fish

cargo build

for domain in (cat domains)
    echo ../target/debug/plot-dnstap --height=20 --output=images/server/ --single-file /mnt/data/Downloads/dnscaptures-2019-11-18-full-rescan/extracted/0/$domain/*-{5,6,7,8,9}-0.dnstap.xz
    echo ../target/debug/plot-dnstap --height=20 --output=images/pi/ --single-file /mnt/data/Downloads/dnscaptures-2019-11-20-pi/extracted/0/$domain/*.dnstap.xz
end | parallel
