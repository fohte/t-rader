# syntax=docker/dockerfile:1

# Keep the Node.js version in sync with .mise.toml.
FROM node:24.18.0-slim AS base
WORKDIR /app
ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH
# Node.js 25+ no longer bundles Corepack: https://github.com/nodejs/corepack
RUN npm install -g corepack@0.35.0 && npm cache clean --force && corepack enable

FROM base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY frontend/package.json frontend/
COPY agent/package.json agent/
RUN --mount=type=cache,id=pnpm-store,target=/pnpm/store \
    pnpm install --frozen-lockfile

# ローカル開発用ステージ (docker compose up agent で使用)
FROM base AS dev
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/agent/node_modules ./agent/node_modules
WORKDIR /app/agent
CMD ["pnpm", "dev"]

FROM deps AS builder
COPY . .
RUN pnpm --filter agent build

# runAsUser/runAsGroup 1000 (node user) は infra 側 Deployment の securityContext で
# 強制される。kubelet は非数値の USER に対して runAsNonRoot を検証できないため、
# このイメージの node ユーザーの uid/gid 1000 を崩さないこと。
FROM base AS runtime
ENV NODE_ENV=production
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY frontend/package.json frontend/
COPY agent/package.json agent/
RUN --mount=type=cache,id=pnpm-store,target=/pnpm/store \
    pnpm install --frozen-lockfile --prod
COPY --from=builder /app/agent/dist ./agent/dist
COPY agent/drizzle ./agent/drizzle
USER node
WORKDIR /app/agent
EXPOSE 8080
CMD ["node", "--import", "@fohte/service-kit/otel-register", "dist/index.js"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
