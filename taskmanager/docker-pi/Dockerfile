FROM arm32v7/debian:10

# Update system to newest version
#
# Install all needed packages
# `dnsutils` for `dig`
# `fstrm-bin` for fstrm_capture
# `libglvnd0` to fix the missing libGL.so
# `libgl1-mesa-dri` to fix a missing DRI library
# `moreutils` for `ts`
# `ncat` for `nc`
# `psmisc` for `killall`
RUN apt-get update \
    && apt-get -y --no-install-recommends install \
        autoconf \
        automake \
        build-essential \
        chromium \
        dnsutils \
        fish \
        fstrm-bin \
        git \
        libfstrm-dev \
        libgl1-mesa-dri \
        libglvnd0 \
        libssl-dev \
        libtool \
        libyaml-dev \
        make \
        moreutils \
        ncat \
        psmisc \
        python3-requests \
        python3-websocket \
        sudo \
        tcpdump \
        unbound \
    && rm -rf /var/lib/apt/lists/*

# Install stubby
RUN true \
    && cd /tmp \
    && git clone https://github.com/getdnsapi/getdns.git \
    && cd getdns \
    && git checkout master \
    && git submodule update --init \
    && libtoolize -ci \
    && autoreconf -fi \
    && mkdir build \
    && cd build \
    && ../configure --without-libidn --without-libidn2 --enable-stub-only --with-stubby \
    && make -j \
    && make install \
    && ldconfig \
    && true

# Add a non-privileged user to run chrome with
# But give it sudo permissions
# Also setup a directory under /run/user, needed for fish to work
RUN useradd --create-home docker \
    && echo "docker ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers \
    && true

# Configure unbound
COPY unbound.conf /etc/unbound/
# * Create runtime directory for unbound
# * And install the trust anchor
#   unbound-anchor returns 1 if the anchor was updated, but this is not an error case
# * Create server certificates for unbound-control to work
RUN true \
    && mkdir /var/run/unbound \
    && chown unbound:unbound /var/run/unbound \
    && (unbound-anchor -F || true) \
    && sg unbound /usr/sbin/unbound-control-setup -d /etc/unbound/ \
    && unbound-checkconf \
    && true

COPY stubby.yml /etc/stubby/stubby.yml

# Copy needed executables
COPY bin /usr/bin/

# Mount point to export all data
VOLUME [ "/output" ]

# configure the docker user
USER docker
ENV XDG_RUNTIME_DIR=/run/user/1000/

CMD [ "/usr/bin/capture-output.sh", "/usr/bin/run-measurements-in-docker.fish", "/output/website-log.log" ]
