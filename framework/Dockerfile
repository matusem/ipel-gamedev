ARG BUILDER_IMAGE=rust:bookworm
FROM ${BUILDER_IMAGE} AS builder

# Bootstrap tooling only for local builds from plain rust:bookworm.
ARG BUILDER_IMAGE
RUN if [ "$BUILDER_IMAGE" = "rust:bookworm" ]; then \
      apt-get update && apt-get install -y --no-install-recommends nodejs npm \
      && rm -rf /var/lib/apt/lists/*; \
    fi

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    if [ "$BUILDER_IMAGE" = "rust:bookworm" ]; then \
      rustup target add wasm32-unknown-unknown wasm32-wasip1 \
      && cargo install dioxus-cli cargo-component --locked; \
    fi

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY game/ game/
COPY game-wasm-host/ game-wasm-host/
COPY server/ server/
COPY games/tic_tac_toe/rust/ games/tic_tac_toe/rust/
COPY tools/gamedev-cli/ tools/gamedev-cli/
COPY sdk/rust/ sdk/rust/
COPY test.wit ./

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build -p server --release \
    && cp target/release/server /app/server-bin

# Ship tic-tac-toe as the default game bundle
COPY games/tic_tac_toe/manifest.json /app/games-tic-src/
COPY games/tic_tac_toe/client/ /app/games-tic-src/client/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo component build -p tic_tac_toe_component --release \
    && mkdir -p /app/games-bundle/tic_tac_toe/client \
    && cp /app/games-tic-src/manifest.json /app/games-bundle/tic_tac_toe/ \
    && cp -r /app/games-tic-src/client/. /app/games-bundle/tic_tac_toe/client/ \
    && cp target/wasm32-wasip1/release/tic_tac_toe_component.wasm /app/games-bundle/tic_tac_toe/logic.wasm

COPY packages/game-sdk/ /app/packages/game-sdk/
WORKDIR /app/packages/game-sdk
RUN npm ci && npm run build
WORKDIR /app

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

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl docker.io docker-compose-plugin \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN mkdir -p /app/data /app/games

COPY --from=builder /app/server-bin /app/server

COPY --from=builder /app/client-lib/ /app/lib/

COPY --from=builder /app/lobby-dist/ /app/lobby/

COPY --from=builder /app/games-bundle/tic_tac_toe/ /app/games/tic_tac_toe/

COPY platform/ /app/platform/
COPY tools/gamedev-cli/manifest.json /app/tools/gamedev-cli/manifest.json
COPY tools/gamedev-cli/install.ps1 /app/tools/gamedev-cli/install.ps1
COPY tools/gamedev-cli/install.sh /app/tools/gamedev-cli/install.sh
RUN chmod +x /app/tools/gamedev-cli/install.sh

ENV HOST=0.0.0.0
ENV PORT=8080
ENV DATABASE_URL=sqlite:///app/data/app.db
ENV GAMES_DIR=/app/games
ENV LOBBY_DIR=/app/lobby
ENV LIB_DIR=/app/lib
ENV PLATFORM_MANIFEST_DIR=/app/platform
ENV TOOLS_DIR=/app/tools

EXPOSE 8080

CMD ["/app/server"]
