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

    # Start fstrm_capture
    start_fstrm
    # Ports
    # 80: HTTP
    # 443: HTTPs, Tor
    # 853: DoT
    # --relinquish-privileges=root prevents tcpdump from dropping privileges to tcpdump, since this causes some unknown error chown-ing the pcap file
    # This only happens in the docker environment of the dnscapture server, but not locally using podmanq
    sudo tcpdump -i any -f "port 853 or port 80 or port 443" --relinquish-privileges=root -w "/output/website-log.pcap" &
    # Start DNS services
    # echo "Starting client proxy"
    # env SSLKEYLOGFILE=/output/website-log.tlskeys.txt RUST_LOG=info /usr/bin/client -l127.0.0.1:8853 -s1.0.0.1:853 --tls pass &

    # run the experiment
    echo "Start Experiment"
    echo python3 "$SCRIPT" "$DOMAIN"
    python3 "$SCRIPT" "$DOMAIN"
    echo "Done Experiment"
    echo
    sleep 2
    killall stubby # client
    sudo killall fstrm_capture tcpdump
    echo "Kill: " $status
    wait
    sleep 1
    echo

    echo "Successfully finished"
end

pushd /output
run 2>&1
