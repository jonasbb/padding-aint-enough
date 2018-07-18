#!/usr/bin/fish

# parse arguments
set -l URL "$argv[1]"

set --export DISPLAY :0
set -l DNSTAP_SOCK /var/run/unbound/dnstap.sock
set -l SCRIPT (realpath (dirname (status --current-filename))/control-chrome.py)
set -l SPECIAL_URL "file:///"
set -l CHROME_DEBUG_PORT 9229

echo "$DNSTAP_SOCK"
echo "$SCRIPT"
echo

# setup temporary working directory
set -l TMPDIR (mktemp --directory)
pushd "$TMPDIR"

# setup dnstap logging
echo "Setup Logging"
sudo fstrm_capture -t protobuf:dnstap.Dnstap -u "$DNSTAP_SOCK" -w dnstap.log &

# Start chrome process already
set -l CHROME_TMPDIR (mktemp --directory)
# create an empty "First Run" file to prevent chrome from showing the frist run dialog
touch "$CHROME_TMPDIR/First Run"
# Disable the NXDOMAIN hijacking checks (7-15 random TLD lookups)
google-chrome --disable-background-networking --user-data-dir="$CHROME_TMPDIR" --remote-debugging-port="$CHROME_DEBUG_PORT" "$SPECIAL_URL" >/dev/null 2>&1 &

# ensure the fstrm_capture and chrome did start
sleep 3

# setup permission
sudo restorecon -vR "$DNSTAP_SOCK"
sudo chown unbound:unbound "$DNSTAP_SOCK"
sudo chmod ug+w "$DNSTAP_SOCK"
sudo chown jbushart:jbushart dnstap.log
sudo chmod ugo+rw dnstap.log
echo "Done Setup Logging"
echo

# run the experiment
echo "Start Experiment"
echo python3 "$SCRIPT" "$SPECIAL_URL" "$CHROME_DEBUG_PORT" "$URL"
python3 "$SCRIPT" "$SPECIAL_URL" "$CHROME_DEBUG_PORT" "$URL"
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
# ensure fstrm_capture has exited
sleep 2
echo

# copy experiment results
popd
mv --force "$TMPDIR"/website-log.json ./website-log.json
cp --force "$TMPDIR"/dnstap.log ./website-log.dnstap

# cleanup
rm -rf "$TMPDIR" "$CHROME_TMPDIR"
echo
