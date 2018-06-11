#!/usr/bin/fish
# Prepare Unbound, flush+restart to empty cache
sudo unbound-control flush_zone .
sudo unbound-control flush_bogus
sudo unbound-control flush_negative
sudo unbound-control flush_infra all
sudo systemctl restart unbound
sleep 1
sudo unbound-control reload
sleep 2
