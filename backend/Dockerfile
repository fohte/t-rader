# syntax=docker/dockerfile:1

FROM rust:1.93.0-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# bind マウントでソースを参照し、cache マウントでビルドキャッシュを永続化する。
# キャッシュマウントは RUN 終了後にアンマウントされるため、同じ RUN 内で cp する。
RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=bind,source=migrations,target=migrations \
    --mount=type=cache,target=/app/target/,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git/db,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry/,sharing=locked \
    SQLX_OFFLINE=true cargo build --locked --release && \
    cp ./target/release/backend /usr/local/bin/backend

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /usr/local/bin/backend /usr/local/bin/backend
ENTRYPOINT ["backend"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
