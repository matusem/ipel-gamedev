# Prebuilt builder for framework/Dockerfile (linux/arm64 in CI).
# Publishes to ghcr.io/<owner>/<repo>-builder:bookworm
FROM rust:bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends nodejs npm \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown wasm32-wasip1

# Match lobby dioxus = 0.7.3 (see framework/lobby/Cargo.toml).
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install dioxus-cli --locked --version 0.7.3 \
    && cargo install cargo-component --locked \
    && cargo install cargo-chef --locked

WORKDIR /app
