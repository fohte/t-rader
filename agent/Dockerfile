# syntax=docker/dockerfile:1

FROM node:24.18.0-slim AS base
WORKDIR /app
RUN corepack enable

FROM base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

# ローカル開発用ステージ (docker compose up agent で使用)
FROM base AS dev
COPY --from=deps /app/node_modules ./node_modules
CMD ["pnpm", "dev"]

FROM deps AS builder
COPY . .
RUN pnpm run build

# runAsUser/runAsGroup 1000 (node user) は infra 側 Deployment の securityContext で
# 強制される。kubelet は非数値の USER に対して runAsNonRoot を検証できないため、
# このイメージの node ユーザーの uid/gid 1000 を崩さないこと。
FROM base AS runtime
ENV NODE_ENV=production
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile --prod
COPY --from=builder /app/dist ./dist
COPY drizzle ./drizzle
EXPOSE 8080
USER node
CMD ["node", "dist/index.js"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
