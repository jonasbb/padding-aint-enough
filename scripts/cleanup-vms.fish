#!/usr/bin/fish

if test (count $argv) -lt 2
    echo (status --current-filename) "<ssh host> <local dir>"
    echo "Copies all old folders from the VM to the local directory."
    echo "<ssh host> must be the ssh address of the VM and log-in must work without password."
    echo "<local dir> can be any local directory with write permissions."
    exit 1
end
set -l SSHHOST "$argv[1]"
set -l LOCAL_DIR "$argv[2]"
if test ! -e "$LOCAL_DIR"
    echo "'$LOCAL_DIR' does not exist"
    exit 1
end
if test ! -d "$LOCAL_DIR"
    echo "'$LOCAL_DIR' is not a directory"
    exit 1
end
if test ! -w "$LOCAL_DIR"
    echo "'$LOCAL_DIR' is not writable"
    exit 1
end

# get a list of "old" directories, created more than 4 hours ago
for dir in (ssh "$SSHHOST" "find ./dnscaptures/ -type d -mmin +240")
    echo "$dir"
    rsync -avz --remove-source-files "$SSHHOST:$dir" "$LOCAL_DIR"
    # delete empty directory left behind by rsync
    ssh "$SSHHOST" "rmdir $dir"
end

# compress all dnstap and json files
fd --no-ignore-vcs --extension dnstap --extension json --exec xz -9 '{}' \; . "$LOCAL_DIR"
