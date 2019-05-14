#!/usr/bin/env bash
pushd $(dirname $(readlink -f $0))
docker build -f Dockerfile -t debian-rust-nightly --rm .
