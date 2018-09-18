#!/usr/bin/env fish

if test (count $argv) -lt 1
    echo "The required argument for the log file is missing."
    exit 1
end

for FILE in $argv[1..-1]
    # path to experiments folder
    set -l FILE (realpath "$FILE")

    # Remove String IDs which change between runs
    sed -i 's/"\\(targetId\\|frameId\\|requestId\\|scriptId\\|executionContextId\\|loaderId\\)": ".*"\\(,\\?\\)/"\\1": ""\\2/g' "$FILE"
    # Remove Timestamps which change between runs
    sed -i 's/"\\(timestamp\\|wallTime\\)": [[:digit:]]\\+\\(.[[:digit:]]\\+\\)\\?\\(,\\?\\)/"\\1": 0\\3/g' "$FILE"
end
