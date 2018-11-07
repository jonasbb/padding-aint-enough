# README

Build:

```sh
docker build -t dnscapture .
```

Run:

```sh
docker run \
    --privileged \
    -v /sys/fs/cgroup:/sys/fs/cgroup:ro \
    -v (pwd)/out:/output \
    -v /tmp/.X11-unix:/tmp/.X11-unix:ro \
    --dns=127.0.0.1 \
    --shm-size=2g \
    --rm \
    dnscapture
```

* `--privileged` needed for chrome
* `/sys/fs/cgroup` needed for systemd
* `/output` to transfer files
* `/tmp/.X11-unix` needed for chrome
* `--dns=127.0.0.1` force localhost as DNS server, otherwise docker overwrites the setting
* `--shm-size=2g` prevents SIGBUS errors due to too small file systems. [Reference](https://goblincoding.com/2018/02/19/docker-bus-error-no-space-left-on-device/)

The `DISPLAY` variable must be set correctly to start chrome.

## Recording Process

The recording setup it started by a systemd service.
The main service is `capture-dns.service`.
It starts the recording procedure and then shutdowns the container.
It calls the `/usr/bin/run-measurements-in-docker.fish` file.

The dnstap capture is controlled by `dnstap.service`.
It starts before unbound and ensures the `fstrm_capture` runs.
