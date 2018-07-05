#!/usr/bin/env fish

if test (count $argv) -lt 1
    echo "The required argument for the experiments directory is missing."
    exit 1
end
# path to experiments folder
set -l EXPERIMENTS (realpath "$argv[1]")

if test ! \( -e "$EXPERIMENTS" -a -d "$EXPERIMENTS" \)
    echo "The experiments directory at $EXPERIMENTS does not exist."
    exit 1
end

# go to parent directory of script
set -l SCRIPT (realpath (dirname (status --current-filename))/../)

cargo build --release --bin encrypted-dns
# search in experiments folder for all files with json extension and process them
fd --no-ignore --extension json --extension json.xz . "$EXPERIMENTS" --exec ./target/release/encrypted-dns '{}' \;

# copy all files into a single images directory
pushd $EXPERIMENTS
rm -rf ./images
mkdir --parents ./images
for f in ./*/**/*.svg
    cp $f ./images/(echo $f | tr '/' '-' | tail -c+3)
end
