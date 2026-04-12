# syntax=docker/dockerfile:1

FROM node:24.14.0-slim AS builder
WORKDIR /app
RUN corepack enable
COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY . .
RUN pnpm run build

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
ENV API_BACKEND_URL=http://localhost:3000
ENV NGINX_RESOLVER=127.0.0.11

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
