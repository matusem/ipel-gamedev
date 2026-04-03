FROM rust:bookworm AS builder

RUN rustup target add wasm32-wasip1 \
    && cargo install cargo-component --locked

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY game/ game/
COPY server/ server/
COPY tic_tac_toe/ tic_tac_toe/
COPY wasm/ wasm/
COPY wasm_game/ wasm_game/
COPY test.wit ./

RUN cargo component build -p wasm --release
RUN cargo build -p server --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/server /app/server
COPY --from=builder /app/target/wasm32-wasip1/release/wasm.wasm /app/wasm.wasm

ENV HOST=0.0.0.0
ENV PORT=8080
ENV WASM_PATH=/app/wasm.wasm

EXPOSE 8080

CMD ["/app/server"]
