FROM fedora:31

# Copy modified unbound rpms
COPY rpms /rpms

# Update system to newest version
#
# Install all needed packages
# `iproute` for `ss`
# `libglvnd-glx` to fix the missing libGL.so
# `mesa-dri-drivers` to fix a missing DRI library
# `moreutils` for `ts`
# `nmap-ncat` for `nc`
# `psmisc` for `killall`
RUN dnf -y --setopt=tsflags=nodocs update \
    && dnf -y --setopt=tsflags=nodocs install \
        /usr/bin/dig \
        /usr/bin/xz \
        autoconf \
        firefox \
        fish \
        fstrm-devel \
        git \
        iproute \
        libglvnd-glx \
        libtool \
        libyaml-devel \
        make \
        mesa-dri-drivers \
        moreutils \
        nmap-ncat \
        openssl-devel \
        psmisc \
        python3-selenium \
        sudo \
        tcpdump \
        wget \
        /rpms/python?-unbound-1*.rpm \
        /rpms/unbound-1*.rpm \
        /rpms/unbound-libs-1*.rpm \
    && dnf clean all \
    && pip install \
        stem \
        tbselenium \
    && cd /tmp \
    # Replace the new tab and browser start page with about:blank
    && sed -i s@https://start.fedoraproject.org/@about:blank@g /usr/lib64/firefox/browser/defaults/preferences/firefox-redhat-default-prefs.js \
    && wget https://github.com/mozilla/geckodriver/releases/download/v0.26.0/geckodriver-v0.26.0-linux64.tar.gz \
    && tar -xvf geckodriver-*.tar.gz \
    && mv geckodriver /usr/local/bin/ \
    && rm geckodriver-*.tar.gz \
    && true

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
    && make \
    && make install \
    && ldconfig \
    && cd / \
    && rm -rf /tmp/getdns \
    && true

# Add a non-privileged user to run chrome with
# But give it sudo permissions
# Also setup a directory under /run/user, needed for fish to work
RUN useradd --create-home docker \
    && echo "docker ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers \
    && mkdir /run/user/`id -u docker` \
    && chmod -R 700 /run/user/`id -u docker` \
    && chown -R docker:docker /run/user/`id -u docker` \
    && true

# Install Tor Browser Bundle
RUN true \
    && cd /tmp \
    && wget https://www.torproject.org/dist/torbrowser/9.0.3/tor-browser-linux64-9.0.3_en-US.tar.xz \
    && tar -xvf tor-browser-linux64-*.tar.xz \
    && mv tor-browser_en-US /opt \
    && rm -rf tor-browser-linux64-*.tar.xz \
    && chown -R docker:docker /opt/tor-browser_en-US \
    && true

COPY stubby.yml /etc/stubby/stubby.yml
# Configure unbound
COPY unbound.conf /etc/unbound/
# Create server certificates for unbound-control to work
RUN sg unbound /usr/sbin/unbound-control-setup -d /etc/unbound/

# Copy needed executables
COPY bin /usr/bin/

# Mount point to export all data
VOLUME [ "/output" ]

# configure the docker user
USER docker
ENV XDG_RUNTIME_DIR=/run/user/1000/

CMD [ "/usr/bin/capture-output.sh", "/usr/bin/run-measurements-in-docker.fish", "/output/website-log.log" ]
