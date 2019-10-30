#!/usr/bin/fish
pushd (dirname (status --current-filename))
set -l PREFETCH_DOMAINS /output/prefetch-domains.txt

# flush old cache entries
sudo unbound-control start
sleep 1
# Reload with new data, run double to ensure no SERVFAILS or similar are returned
# Second one should be very quick anyways
dns-par --file "$PREFETCH_DOMAINS" --limit-parallel 20 --servers "127.0.0.1"
dns-par --file "$PREFETCH_DOMAINS" --limit-parallel 20 --servers "127.0.0.1"
# Dump to stdout
sudo unbound-control dump_cache > /output/cache.dump.new
