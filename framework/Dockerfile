FROM rust:bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends nodejs npm \
    && rm -rf /var/lib/apt/lists/*

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    rustup target add wasm32-wasip1 wasm32-unknown-unknown \
    && cargo install cargo-component --locked \
    && cargo install dioxus-cli --locked

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY game/ game/
COPY game-wasm-host/ game-wasm-host/
COPY server/ server/
COPY games/tic_tac_toe/rust/ games/tic_tac_toe/rust/
COPY test.wit ./

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo component build -p tic_tac_toe_component --release \
    && cargo build -p server --release \
    && cp target/release/server /app/server-bin \
    && cp target/wasm32-wasip1/release/tic_tac_toe_component.wasm /app/wasm-bin.wasm

COPY lobby/ lobby/
WORKDIR /app/lobby
RUN npm ci && npm run build:css
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/lobby/target \
    dx build --platform web --release \
    && cp -r target/dx/lobby/release/web/public /app/lobby-dist

WORKDIR /app

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN mkdir -p /app/data

COPY --from=builder /app/server-bin /app/server

COPY --from=builder /app/wasm-bin.wasm /app/games/tic_tac_toe/logic.wasm
COPY games/tic_tac_toe/manifest.json /app/games/tic_tac_toe/manifest.json
COPY games/tic_tac_toe/client/ /app/games/tic_tac_toe/client/

COPY client-lib/ /app/lib/

COPY --from=builder /app/lobby-dist/ /app/lobby/

ENV HOST=0.0.0.0
ENV PORT=8080
ENV DATABASE_URL=sqlite:///app/data/app.db
ENV GAMES_DIR=/app/games
ENV LOBBY_DIR=/app/lobby
ENV LIB_DIR=/app/lib

EXPOSE 8080

CMD ["/app/server"]
