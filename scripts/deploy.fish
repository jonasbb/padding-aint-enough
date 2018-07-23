#!/usr/bin/fish
pushd (dirname (status --current-filename))/..

# Scripts working dir is the checkout directory of git
function do
    set -l OUTDIR "$argv[1]"
    cp --recursive scripts ./taskmanager/tasks.db $OUTDIR
    xsv select 2 ./alexa-top1m.????????T????.csv >$OUTDIR/alexa-top1m.txt
    xsv select 2 ./alexa-top1m.????????T????.csv | head -10000 >$OUTDIR/alexa-top10k.txt
end

do /mnt/data/vms/share/
do /mnt/data/Downloads/new-task-setup/
