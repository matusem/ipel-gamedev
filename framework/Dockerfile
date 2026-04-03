FROM rust:bookworm AS builder

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    rustup target add wasm32-wasip1 \
    && cargo install cargo-component --locked

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY game/ game/
COPY server/ server/
COPY tic_tac_toe/ tic_tac_toe/
COPY wasm/ wasm/
COPY test.wit ./

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo component build -p wasm --release \
    && cargo build -p server --release \
    && cp target/release/server /app/server-bin \
    && cp target/wasm32-wasip1/release/wasm.wasm /app/wasm-bin.wasm

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/server-bin /app/server
COPY --from=builder /app/wasm-bin.wasm /app/wasm.wasm

ENV HOST=0.0.0.0
ENV PORT=8080
ENV WASM_PATH=/app/wasm.wasm

EXPOSE 8080

CMD ["/app/server"]
