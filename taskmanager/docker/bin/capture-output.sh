#!/usr/bin/bash

stdbuf -i0 -o0 -e0 "$1" | ts | tee "$2"
