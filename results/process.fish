#!/usr/bin/env fish

# change into dir of this script
pushd (dirname (status --current-filename))

./stats-to-per-tld-pickle.py ./*/statistics*.k*.csv

for pickle in ./*/*.pickle
    ../plot/src/percentage_stacked_area_chart.py "$pickle"
end
