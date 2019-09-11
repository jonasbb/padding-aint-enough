#!/usr/bin/env bash
pushd "$(dirname "$(readlink -f "$0")")" || exit
docker build -f Dockerfile -t debian-rust-nightly --rm .
