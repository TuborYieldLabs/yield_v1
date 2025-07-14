FROM rust:1.87.0

ARG DEBIAN_FRONTEND=noninteractive
ARG SOLANA_CLI="v2.2.2"
ARG ANCHOR_CLI="v0.31.1"
ARG NODE_VERSION="23.10.0"

ENV HOME="/root"
ENV PATH="${HOME}/.cargo/bin:${HOME}/.local/share/solana/install/active_release/bin:${HOME}/.nvm/versions/node/v${NODE_VERSION}/bin:${PATH}"
ENV NVM_DIR="${HOME}/.nvm"

# Install base dependencies
RUN mkdir -p /workdir /tmp && \
    apt-get update -qq && apt-get upgrade -qq && \
    apt-get install -qq --no-install-recommends \
    build-essential git curl wget jq pkg-config python3-pip \
    libssl-dev libudev-dev ca-certificates gnupg && \
    rm -rf /var/lib/apt/lists/*

# Optional: install libssl1.1 if absolutely needed
RUN wget http://archive.ubuntu.com/ubuntu/pool/main/o/openssl/libssl1.1_1.1.1f-1ubuntu2_amd64.deb && \
    dpkg -i libssl1.1_1.1.1f-1ubuntu2_amd64.deb || true

# Install Rust components
RUN rustup component add rustfmt clippy

# Install Solana CLI
RUN sh -c "$(curl -sSfL https://release.anza.xyz/${SOLANA_CLI}/install)"

# Install Anchor CLI (without avm)
RUN cargo install --git https://github.com/coral-xyz/anchor --tag ${ANCHOR_CLI} anchor-cli

# Install Node using NVM
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash && \
    . "$NVM_DIR/nvm.sh" && \
    nvm install v${NODE_VERSION} && \
    nvm alias default v${NODE_VERSION} && \
    npm install -g yarn ts-mocha

# Set workdir and shell
WORKDIR /workdir
CMD ["/bin/bash"]
