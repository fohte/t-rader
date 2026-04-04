# syntax=docker/dockerfile:1

FROM oven/bun:1.3.9-slim AS builder
WORKDIR /app
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile
COPY . .
RUN bun run build

FROM nginx:stable-alpine AS runtime
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf.template /etc/nginx/templates/default.conf.template
EXPOSE 80
# nginx 公式イメージの docker-entrypoint.sh が /etc/nginx/templates/*.template を
# envsubst で処理して /etc/nginx/conf.d/ に出力する。
# NGINX_ENVSUBST_TEMPLATE_SUFFIX はデフォルトで .template。
# NGINX_ENVSUBST_OUTPUT_DIR はデフォルトで /etc/nginx/conf.d。
# nginx 変数 ($host, $uri 等) を壊さないよう、置換対象を明示的に指定する。
ENV NGINX_ENVSUBST_FILTER=API_BACKEND_URL|NGINX_RESOLVER
# 実行時に以下の環境変数を設定すること (docker run -e, docker-compose, k8s manifest 等):
#   API_BACKEND_URL  - backend API の URL (例: http://backend:3000)
#   NGINX_RESOLVER   - nginx の DNS リゾルバ (例: 127.0.0.11)

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
