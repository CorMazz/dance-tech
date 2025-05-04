FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

# Include lld linker to improve build times either by using environment variable
# RUSTFLAGS="-C link-arg=-fuse-ld=lld" or with Cargo's configuration file (i.e see .cargo/config.toml).
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
   && apt-get -y install clang lld \
   && apt-get autoremove -y && apt-get clean -y

USER vscode
ENV RUSTFLAGS="-C link-arg=-fuse-ld=lld"
ARG RUST_VERSION=1.86

# Set the specified Rust toolchain as the default
RUN rustup default ${RUST_VERSION}
RUN rustup component add clippy rust-src rustfmt

RUN cargo install sqlx-cli
RUN cargo install cargo-watch

