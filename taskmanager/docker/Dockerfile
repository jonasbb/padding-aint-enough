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
# Copy needed executables
COPY bin /usr/bin/
COPY ./capture-dns.service /etc/systemd/system/
RUN systemctl enable capture-dns

# Add a non-privileged user to run chrome with
# But give it sudo permissions
RUN useradd --create-home docker \
    && echo "docker ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers

# # Install all needed packages
# # `iproute` for `ss`
# # `bind-utils` for `dig`
# RUN dnf -y --setopt=tsflags=nodocs install \
#     dnf-plugins-core \
#     fedora-workstation-repositories \
#     && dnf config-manager --set-enabled google-chrome \
#     && dnf -y --setopt=tsflags=nodocs install \
#     bind-utils \
#     fish \
#     fstrm-devel \
#     google-chrome-stable \
#     iproute \
#     libglvnd-glx \
#     mesa-dri-drivers \
#     psmisc \
#     python3-websocket-client \
#     sudo \
#     systemd \
#     /rpms/unbound*.rpm \
#     && dnf clean all

# Mount point to export all data
VOLUME [ "/output" ]
