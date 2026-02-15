# syntax=docker/dockerfile:1

FROM oven/bun:1.3.8-slim AS builder
WORKDIR /app
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile
COPY . .
# build スクリプトが未定義の場合は空の dist を作成
RUN bun run build 2>/dev/null || mkdir -p dist

FROM nginx:stable-alpine AS runtime
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
