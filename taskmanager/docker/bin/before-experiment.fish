#!/usr/bin/fish

# This file is called from control-chrome directly before the website is loaded

echo "Flush"
# Prepare Unbound, flush+restart to empty cache
sudo unbound-control flush_zone .
sudo unbound-control flush_bogus
sudo unbound-control flush_negative
sudo unbound-control flush_infra all
# sudo systemctl restart unbound

echo "Load cache file"
cat /output/cache.dump | sudo unbound-control load_cache

echo "start.example marker query"
dig @127.0.0.1 +tries=1 start.example. >/dev/null 2>&1
