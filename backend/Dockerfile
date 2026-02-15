# syntax=docker/dockerfile:1

FROM rust:1.92.0-slim AS chef
RUN cargo install cargo-chef cargo-watch
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
ENTRYPOINT ["backend"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
