#!/usr/bin/fish
pushd (dirname (status --current-filename))/..
set -l OUTDIR /mnt/data/vms/share/

# Scripts working dir is the checkout directory of git

cp --recursive scripts python record-websites.fish $OUTDIR
xsv select 2 ./alexa-top1m.????????T????.csv >$OUTDIR/alexa-top1m.txt

