FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

# Include lld linker to improve build times either by using environment variable
# RUSTFLAGS="-C link-arg=-fuse-ld=lld" or with Cargo's configuration file (i.e see .cargo/config.toml).
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
   && apt-get -y install clang lld \
   && apt-get autoremove -y && apt-get clean -y

# USER vscode  # If using vscode devcontainer development, uncomment this line
ENV RUSTFLAGS="-C link-arg=-fuse-ld=lld"
ARG RUST_VERSION=1.86

# Set the specified Rust toolchain as the default
RUN rustup default ${RUST_VERSION}
RUN rustup component add clippy rust-src rustfmt rust-analyzer

RUN cargo install sqlx-cli
RUN cargo install cargo-watch

# If using neovim keep the following uncommented

# RUN apt-get -y install glibc \
#   && apt-get autoremove -y \
#   && apt-get clean -y

RUN curl -LO https://github.com/neovim/neovim/releases/latest/download/nvim-linux-x86_64.tar.gz
RUN sudo rm -rf /opt/nvim
RUN sudo tar -C /opt -xzf nvim-linux-x86_64.tar.gz
RUN rm nvim-linux-x86_64.tar.gz
RUN echo 'export PATH="$PATH:/opt/nvim-linux-x86_64/bin"' >> root/.bashrc
RUN echo 'alias nv="nvim"' >> root/.bashrc

# Use bash for the shell
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Create a script file sourced by both interactive and non-interactive bash shells
ENV BASH_ENV="/root/.bash_env"
RUN touch "${BASH_ENV}"
RUN echo '. "${BASH_ENV}"' >> root/.bashrc

# Download and install nvm
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.2/install.sh | PROFILE="${BASH_ENV}" bash
RUN echo node > .nvmrc
RUN nvm install node --lts
