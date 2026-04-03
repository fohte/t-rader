# syntax=docker/dockerfile:1

FROM oven/bun:1.3.9-slim AS builder
WORKDIR /app
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile
COPY . .
RUN bun run build

FROM nginx:stable-alpine AS runtime
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
