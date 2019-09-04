FROM debian:9

# The Rust toolchain to use when building our image.  Set by `hooks/build`.
ARG TOOLCHAIN=nightly-2019-08-10

RUN apt-get update && \
    apt-get install -y \
    build-essential \
    curl \
    liblzma-dev \
    libpcap-dev \
    libpq-dev \
    libssl-dev \
    sudo \
    && \
    apt-get clean && rm -rf /var/lib/apt/lists/* && \
    useradd rust --user-group --create-home --shell /bin/bash --groups sudo

# RUN apt-get update && apt-get install -y curl

# Run all further code as user `rust`, and create our working directories
# as the appropriate user.
USER rust
RUN mkdir -p /home/rust/libs /home/rust/src

ENV PATH=/home/rust/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- -y --default-toolchain $TOOLCHAIN && \
    rustup target add x86_64-unknown-linux-gnu

WORKDIR /home/rust/src
