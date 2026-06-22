# syntax=docker/dockerfile:1
ARG BUILDER_IMAGE=rust:bookworm
FROM ${BUILDER_IMAGE} AS chef

# Bootstrap tooling only for local builds from plain rust:bookworm.
ARG BUILDER_IMAGE
RUN if [ "$BUILDER_IMAGE" = "rust:bookworm" ]; then \
      apt-get update && apt-get install -y --no-install-recommends nodejs npm \
      && rm -rf /var/lib/apt/lists/* \
      && rustup target add wasm32-unknown-unknown wasm32-wasip1 \
      && cargo install dioxus-cli cargo-component cargo-chef --locked; \
    fi

WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY game/ game/
COPY bot/ bot/
COPY game-wasm-host/ game-wasm-host/
COPY bot-wasm-host/ bot-wasm-host/
COPY server/ server/
COPY games/tic_tac_toe/rust/ games/tic_tac_toe/rust/
COPY games/tic_tac_toe_bot/rust/ games/tic_tac_toe_bot/rust/
COPY tools/gamedev-cli/ tools/gamedev-cli/
COPY sdk/rust/ sdk/rust/
COPY test.wit bot.wit ./
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

COPY --from=planner /app/recipe.json recipe.json
COPY Cargo.toml Cargo.lock ./
COPY game/Cargo.toml game/Cargo.toml
COPY bot/Cargo.toml bot/Cargo.toml
COPY game-wasm-host/Cargo.toml game-wasm-host/Cargo.toml
COPY bot-wasm-host/Cargo.toml bot-wasm-host/Cargo.toml
COPY server/Cargo.toml server/Cargo.toml
COPY games/tic_tac_toe/rust/logic/Cargo.toml games/tic_tac_toe/rust/logic/Cargo.toml
COPY games/tic_tac_toe/rust/component/Cargo.toml games/tic_tac_toe/rust/component/Cargo.toml
COPY games/tic_tac_toe_bot/rust/logic/Cargo.toml games/tic_tac_toe_bot/rust/logic/Cargo.toml
COPY games/tic_tac_toe_bot/rust/component/Cargo.toml games/tic_tac_toe_bot/rust/component/Cargo.toml
COPY tools/gamedev-cli/Cargo.toml tools/gamedev-cli/Cargo.toml
COPY sdk/rust/shared-types/Cargo.toml sdk/rust/shared-types/Cargo.toml
COPY sdk/rust/shared/Cargo.toml sdk/rust/shared/Cargo.toml
COPY sdk/rust/bevy/Cargo.toml sdk/rust/bevy/Cargo.toml
COPY sdk/rust/dioxus/Cargo.toml sdk/rust/dioxus/Cargo.toml
COPY scripts/docker-cargo-chef-stubs.sh scripts/docker-cargo-chef-stubs.sh
RUN chmod +x scripts/docker-cargo-chef-stubs.sh \
    && scripts/docker-cargo-chef-stubs.sh

# Dependency layer: cached when Cargo.lock / recipe.json are unchanged.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json -p server

COPY game/ game/
COPY bot/ bot/
COPY game-wasm-host/ game-wasm-host/
COPY bot-wasm-host/ bot-wasm-host/
COPY server/ server/
COPY games/tic_tac_toe/rust/ games/tic_tac_toe/rust/
COPY games/tic_tac_toe_bot/rust/ games/tic_tac_toe_bot/rust/
COPY tools/gamedev-cli/ tools/gamedev-cli/
COPY sdk/rust/ sdk/rust/
COPY test.wit bot.wit ./

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build -p server --release \
    && cp target/release/server /app/server-bin

# Ship tic-tac-toe as the default game bundle
COPY games/tic_tac_toe/manifest.json /app/games-tic-src/
COPY games/tic_tac_toe/contract.json /app/games-tic-src/
COPY games/tic_tac_toe/client/ /app/games-tic-src/client/
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo component build -p tic_tac_toe_component --release \
    && mkdir -p /app/games-bundle/tic_tac_toe/client \
    && cp /app/games-tic-src/manifest.json /app/games-bundle/tic_tac_toe/ \
    && cp /app/games-tic-src/contract.json /app/games-bundle/tic_tac_toe/ \
    && cp -r /app/games-tic-src/client/. /app/games-bundle/tic_tac_toe/client/ \
    && cp target/wasm32-wasip1/release/tic_tac_toe_component.wasm /app/games-bundle/tic_tac_toe/logic.wasm

# Ship tic-tac-toe bot as the default bot bundle
COPY games/tic_tac_toe_bot/manifest.json /app/bots-tic-src/
COPY games/tic_tac_toe_bot/contract/contract.json /app/bots-tic-src/contract.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo component build -p tic_tac_toe_bot_component --release \
    && mkdir -p /app/bots-bundle/tic_tac_toe_bot \
    && cp /app/bots-tic-src/manifest.json /app/bots-bundle/tic_tac_toe_bot/ \
    && cp /app/bots-tic-src/contract.json /app/bots-bundle/tic_tac_toe_bot/contract.json \
    && cp target/wasm32-wasip1/release/tic_tac_toe_bot_component.wasm /app/bots-bundle/tic_tac_toe_bot/bot.wasm

COPY packages/game-sdk/ /app/packages/game-sdk/
WORKDIR /app/packages/game-sdk
RUN --mount=type=cache,target=/root/.npm \
    npm ci && npm run build
WORKDIR /app

COPY lobby/ lobby/
WORKDIR /app/lobby
RUN --mount=type=cache,target=/root/.npm \
    npm ci && npm run build:css
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    dx build --platform web --release \
    && cp -r target/dx/lobby/release/web/public /app/lobby-dist

WORKDIR /app

FROM debian:bookworm-slim

ARG COMPOSE_VERSION=2.32.4
ARG TARGETARCH

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl docker.io \
    && mkdir -p /usr/local/lib/docker/cli-plugins \
    && case "${TARGETARCH}" in \
         amd64) COMPOSE_ARCH=x86_64 ;; \
         arm64) COMPOSE_ARCH=aarch64 ;; \
         *) echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;; \
       esac \
    && curl -fsSL \
         "https://github.com/docker/compose/releases/download/v${COMPOSE_VERSION}/docker-compose-linux-${COMPOSE_ARCH}" \
         -o /usr/local/lib/docker/cli-plugins/docker-compose \
    && chmod +x /usr/local/lib/docker/cli-plugins/docker-compose \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN mkdir -p /app/data /app/games /app/bots

COPY --from=builder /app/server-bin /app/server

COPY --from=builder /app/client-lib/ /app/lib/

COPY --from=builder /app/lobby-dist/ /app/lobby/

COPY --from=builder /app/games-bundle/tic_tac_toe/ /app/games/tic_tac_toe/

COPY --from=builder /app/bots-bundle/tic_tac_toe_bot/ /app/bots/tic_tac_toe_bot/

COPY platform/ /app/platform/
COPY tools/gamedev-cli/manifest.json /app/tools/gamedev-cli/manifest.json
COPY tools/gamedev-cli/install.ps1 /app/tools/gamedev-cli/install.ps1
COPY tools/gamedev-cli/install.sh /app/tools/gamedev-cli/install.sh
# Populated by release CI with versioned zip/tar.gz bundles (see release-deploy workflow).
COPY release-artifacts/tools/ /app/tools/gamedev-cli/
RUN chmod +x /app/tools/gamedev-cli/install.sh

ENV HOST=0.0.0.0
ENV PORT=8080
ENV DATABASE_URL=sqlite:///app/data/app.db
ENV GAMES_DIR=/app/games
ENV BOTS_DIR=/app/bots
ENV LOBBY_DIR=/app/lobby
ENV LIB_DIR=/app/lib
ENV PLATFORM_MANIFEST_DIR=/app/platform
ENV TOOLS_DIR=/app/tools

EXPOSE 8080

CMD ["/app/server"]
