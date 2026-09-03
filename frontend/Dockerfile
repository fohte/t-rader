# syntax=docker/dockerfile:1

FROM node:24.20.0-slim AS builder
WORKDIR /app
RUN corepack enable
# workspace member を追加したら、この block と agent/Dockerfile の deps/runtime stage
# 両方に COPY <pkg>/package.json <pkg>/ を追記すること。
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY frontend/package.json frontend/
COPY agent/package.json agent/
RUN pnpm install --frozen-lockfile
COPY . .
RUN pnpm --filter frontend build

FROM nginx:stable-alpine AS runtime
COPY --from=builder /app/frontend/dist /usr/share/nginx/html
COPY frontend/nginx.conf.template /etc/nginx/templates/default.conf.template
EXPOSE 80
# nginx 公式イメージの docker-entrypoint.sh が /etc/nginx/templates/*.template を
# envsubst で処理して /etc/nginx/conf.d/ に出力する。
# NGINX_ENVSUBST_TEMPLATE_SUFFIX はデフォルトで .template。
# NGINX_ENVSUBST_OUTPUT_DIR はデフォルトで /etc/nginx/conf.d。
# nginx 変数 ($host, $uri 等) を壊さないよう、置換対象を明示的に指定する。
ENV NGINX_ENVSUBST_FILTER=API_BACKEND_URL|NGINX_RESOLVER

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
