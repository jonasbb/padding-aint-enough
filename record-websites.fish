#!/usr/bin/fish

set -l DNSTAP_SOCK /var/run/unbound/dnstap.sock
set -l SCRIPT (realpath (dirname (status --current-filename))/traffic-logging/control-chrome.py)

echo $DNSTAP_SOCK
echo $SCRIPT
echo

for i in (seq $argv[1])
    # setup temporary working directory
    set -l TMPDIR (mktemp --directory)
    pushd $TMPDIR

    # setup dnstap logging
    echo "Setup Logging"
    sudo fstrm_capture -t protobuf:dnstap.Dnstap -u $DNSTAP_SOCK -w dnstap.log &
    sleep 3
    # setup permission
    sudo restorecon -vR $DNSTAP_SOCK
    sudo chown unbound:unbound $DNSTAP_SOCK
    sudo chmod ug+w $DNSTAP_SOCK
    sudo chown jbushart:jbushart dnstap.log
    sudo chmod ugo+rw dnstap.log
    echo "Done Setup Logging"
    echo

    # Prepare Unbound, flush+restart to empty cache
    sudo unbound-control flush_zone .
    sudo unbound-control flush_bogus
    sudo unbound-control flush_negative
    sudo unbound-control flush_infra all
    sudo systemctl restart unbound
    sleep 2
    sudo unbound-control reload
    sleep 2

    # before experiment
    echo "Before Experiment"
    dig @127.0.0.1 start.example. >/dev/null 2>&1
    echo
    # run the experiment
    echo "Start Experiment"
    echo python3 $SCRIPT $argv[2]
    python3 $SCRIPT $argv[2]
    echo "Done Experiment"
    echo
    # after experiment
    echo "After Experiment"
    dig @127.0.0.1 end.example. >/dev/null 2>&1
    sleep 2
    sudo killall fstrm_capture
    echo "Kill: " $status
    sleep 2
    echo

    # copy experiment results
    popd
    mv $TMPDIR/website-log.json ./website-log-$i.json
    cp $TMPDIR/dnstap.log ./website-log-$i.dnstap

    # cleanup
    rm -rf $TMPDIR
    echo
end
