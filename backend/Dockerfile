# syntax=docker/dockerfile:1

FROM rust:1.93.1-slim AS build
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev curl
WORKDIR /app

# bind マウントでソースを参照し、cache マウントでビルドキャッシュを永続化する。
# bind マウントは read-only のため、target ディレクトリをその外に配置する。
# キャッシュマウントは RUN 終了後にアンマウントされるため、同じ RUN 内で cp する。
RUN --mount=type=bind,target=. \
    --mount=type=cache,target=/build/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git/db,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    CARGO_TARGET_DIR=/build/target cargo build --locked --release && \
    cp /build/target/release/backend /usr/local/bin/backend

# ローカル開発用ステージ (docker compose up backend で使用)
FROM build AS dev
RUN cargo install cargo-watch

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /usr/local/bin/backend /usr/local/bin/backend
ENTRYPOINT ["backend"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
