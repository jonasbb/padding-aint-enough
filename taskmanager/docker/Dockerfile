FROM fedora:29

# Copy modified unbound rpms
COPY rpms /rpms

# Update system to newest version
#
# Install all needed packages
# `bind-utils` for `dig`
# `iproute` for `ss`
# `libglvnd-glx` to fix the missing libGL.so
# `mesa-dri-drivers` to fix a missing DRI library
# `moreutils` for `ts`
# `nmap-ncat` for nc
# `psmisc` for `killall`
RUN dnf -y --setopt=tsflags=nodocs update \
    && dnf -y --setopt=tsflags=nodocs install \
        dnf-plugins-core \
        fedora-workstation-repositories \
    && dnf config-manager --set-enabled google-chrome \
    && dnf -y --setopt=tsflags=nodocs install \
        autoconf \
        bind-utils \
        fish \
        fstrm-devel \
        git \
        google-chrome-stable \
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
        python3-requests \
        python3-websocket-client \
        sudo \
        tcpdump \
        /rpms/unbound-1*.rpm \
        /rpms/unbound-libs-1*.rpm \
        /rpms/python?-unbound-1*.rpm \
    && dnf clean all

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
    && true

# Add a non-privileged user to run chrome with
# But give it sudo permissions
# Also setup a directory under /run/user, needed for fish to work
RUN useradd --create-home docker \
    && echo "docker ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers \
    && mkdir /run/user/`id -u docker` \
    && chmod -R 700 /run/user/`id -u docker` \
    && chown -R docker:docker /run/user/`id -u docker`

# Configure unbound
COPY unbound.conf /etc/unbound/
# Create server certificates for unbound-control to work
RUN sg unbound /usr/sbin/unbound-control-setup -d /etc/unbound/

# Copy needed executables
COPY bin /usr/bin/

# Mount point to export all data
VOLUME [ "/output", "/tmp/.X11-unix" ]

# configure the docker user
USER docker
ENV XDG_RUNTIME_DIR=/run/user/1000/

CMD [ "/usr/bin/capture-output.sh", "/usr/bin/run-measurements-in-docker.fish", "/output/website-log.log" ]
