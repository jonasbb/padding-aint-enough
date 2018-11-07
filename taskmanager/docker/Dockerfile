FROM fedora:28

# Update system to newest version
RUN dnf -y --setopt=tsflags=nodocs update && dnf clean all

# Prepare environment for systemd
RUN (cd /lib/systemd/system/sysinit.target.wants/; for i in *; do [ $i == systemd-tmpfiles-setup.service ] || rm -f $i; done); \
rm -f /lib/systemd/system/multi-user.target.wants/*; \
rm -f /etc/systemd/system/*.wants/*; \
rm -f /lib/systemd/system/local-fs.target.wants/*; \
rm -f /lib/systemd/system/sockets.target.wants/*udev*; \
rm -f /lib/systemd/system/sockets.target.wants/*initctl*; \
rm -f /lib/systemd/system/basic.target.wants/*; \
rm -f /lib/systemd/system/anaconda.target.wants/*;
ENV container docker
VOLUME [ "/sys/fs/cgroup" ]
CMD [ "/usr/sbin/init" ]

# Copy modified unbound rpms
COPY rpms /rpms
# Install all needed packages
# `iproute` for `ss`
# `bind-utils` for `dig`
# `daemonize` for `daemonize`
# `psmisc` for `killall`
# `moreutils` for `ts`
RUN dnf -y --setopt=tsflags=nodocs install \
    dnf-plugins-core \
    fedora-workstation-repositories \
    && dnf config-manager --set-enabled google-chrome \
    && dnf -y --setopt=tsflags=nodocs install \
    bind-utils \
    daemonize \
    fish \
    fstrm-devel \
    google-chrome-stable \
    iproute \
    libglvnd-glx \
    mesa-dri-drivers \
    moreutils \
    psmisc \
    python3-websocket-client \
    sudo \
    /rpms/unbound*.rpm \
    && dnf clean all

# Configure unbound
COPY unbound.conf /etc/unbound/
# Copy needed executables
COPY bin /usr/bin/
# Setup systemd units
COPY [ "./capture-dns.service", "./dnstap.service", "/etc/systemd/system/" ]
RUN systemctl enable capture-dns unbound dnstap

# Add a non-privileged user to run chrome with
# But give it sudo permissions
RUN useradd --create-home docker \
    && echo "docker ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers

# Mount point to export all data
VOLUME [ "/output", "/tmp/.X11-unix" ]
