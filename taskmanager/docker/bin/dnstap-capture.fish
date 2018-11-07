#!/usr/bin/env fish

set -l DNSTAP_SOCK /var/run/unbound/dnstap.sock

# make directory if not exists
mkdir --mode=777 -p /output/(cat /output/taskname)/

daemonize -p /run/fstrm_capture.pid \
    /usr/bin/fstrm_capture -t protobuf:dnstap.Dnstap -u "$DNSTAP_SOCK" -w /output/(cat /output/taskname)/dnstap.log &

# Wait until socket upens up
while [ ! -e "$DNSTAP_SOCK" ];
    sleep .1
    echo "Waiting for socket $DNSTAP_SOCK"
end
echo "Found socket $DNSTAP_SOCK"
chmod uga+rwx $DNSTAP_SOCK
