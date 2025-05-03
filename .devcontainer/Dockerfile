FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

# Include lld linker to improve build times either by using environment variable
# RUSTFLAGS="-C link-arg=-fuse-ld=lld" or with Cargo's configuration file (i.e see .cargo/config.toml).
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
   && apt-get -y install clang lld \
   && apt-get autoremove -y && apt-get clean -y

RUN cargo install sqlx-cli
RUN cargo install cargo-watch

ARG RUST_VERSION=1.86

# Install the specified Rust toolchain if it's not already present
RUN rustup toolchain install ${RUST_VERSION} --no-self-update

# Set the specified Rust toolchain as the default
RUN rustup default ${RUST_VERSION}
