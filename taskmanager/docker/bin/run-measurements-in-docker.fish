#!/usr/bin/env fish

# This script requires the following setup to run correctly
#
# * `taskname` file, containing the name of the task. This will be used to create an output directory
# * `display` file, containing the X11 display number, see DISPLAY variable
# * `domain` file, containing the domain to load

function start_fstrm
    set -l LOG_FILE /output/website-log.dnstap
    sudo /usr/bin/fstrm_capture -t protobuf:dnstap.Dnstap -u "$DNSTAP_SOCK" -w $LOG_FILE 2>&1 &
    set -g FSTRM_PID %last
    echo "Started fstrm_capture with PID $FSTRM_PID"

    # Wait until socket upens up
    while [ ! -e "$DNSTAP_SOCK" ];
        sleep .1
        echo "Waiting for socket $DNSTAP_SOCK"
    end
    echo "Found socket $DNSTAP_SOCK"
    sudo chmod uga+rwx $DNSTAP_SOCK
    # Wait until output file is created
    while [ ! -e "$DNSTAP_SOCK" ];
        sleep .1
        echo "Waiting for dnstap log file $LOG_FILE"
    end
    echo "Found dnstap log file $LOG_FILE"
    sudo chown docker:docker $LOG_FILE
end

function run
    echo "Now executing" (status --current-filename)

    set -g SCRIPT /usr/bin/control-chrome.py
    set -gx DISPLAY (cat display)
    set -g DOMAIN (cat domain)

    set -g DNSTAP_SOCK /var/run/unbound/dnstap.sock
    set -g SPECIAL_URL "file:///"
    set -g CHROME_DEBUG_PORT 9229

    # Start fstrm_capture
    start_fstrm
    # Start Unbound
    echo "Starting unbound"
    sudo unbound-control start

    # Start chrome process already
    set -l TMPDIR (mktemp --directory)
    set -l CHROME_TMPDIR (mktemp --directory)
    pushd $TMPDIR
    echo "Using temporary directories '$TMPDIR' and '$CHROME_TMPDIR'"
    # create an empty "First Run" file to prevent chrome from showing the frist run dialog
    touch "$CHROME_TMPDIR/First Run"
    echo "Starting Chrome..."
    # Disable the NXDOMAIN hijacking checks (7-15 random TLD lookups)
    google-chrome \
        --disable-background-networking \
        --user-data-dir="$CHROME_TMPDIR" \
        --remote-debugging-port="$CHROME_DEBUG_PORT" \
        "$SPECIAL_URL" \
        >/dev/null 2>&1 &
    set -l CHROME_PID %last
    echo "Started Chrome with PID $CHROME_PID"
    # wait for chrome to open the debug port
    while not nc -z localhost "$CHROME_DEBUG_PORT"
        sleep 0.2
        echo "Waiting on Chrome to start"
    end
    echo "Chrome started"

    # run the experiment
    echo "Start Experiment"
    echo python3 "$SCRIPT" "$SPECIAL_URL" "$CHROME_DEBUG_PORT" "$DOMAIN"
    python3 "$SCRIPT" "$SPECIAL_URL" "$CHROME_DEBUG_PORT" "$DOMAIN"
    echo "Done Experiment"
    echo
    # after experiment
    echo "After Experiment"
    dig @127.0.0.1 +tries=1 A "end.example." >/dev/null 2>&1
    sleep 2
    # Chrome should have exited by now
    killall google-chrome chrome
    sudo killall fstrm_capture
    echo "Kill: " $status
    wait
    sleep 1
    echo

    # copy experiment results
    popd
    mv --force "$TMPDIR"/website-log.json ./website-log.json

    # cleanup
    echo "Cleanup"
    rm -rf "$TMPDIR" "$CHROME_TMPDIR"

    echo "Successfully finished"
end

pushd /output
run 2>&1
