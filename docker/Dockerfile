FROM debian:10

# The Rust toolchain to use when building our image.  Set by `hooks/build`.
ARG TOOLCHAIN=nightly-2019-11-17

RUN true && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        liblzma-dev \
        libpq-dev \
        lld \
        sudo \
    && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    true

ENV PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- -y --default-toolchain $TOOLCHAIN && \
    rustup target add x86_64-unknown-linux-gnu
