FROM docker.io/dockcross/linux-armv6:20261028-1173e18

# Install OpenSSL development libraries for the cross-compilation target
RUN dpkg --add-architecture armhf && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        g++-arm-linux-gnueabihf \
        libclang-dev

# Install Rust via rustup inside the container environment
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable

# Ensure cargo and rustup binaries are available in PATH for all shell sessions
ENV PATH="/root/.cargo/bin:${PATH}"

# Pre-install the target for armv6 inside the image
RUN rustup target add arm-unknown-linux-gnueabihf

# Dockcross mounts the project directory to /build by default
WORKDIR /build
