# syntax=docker/dockerfile:1

<<<<<<< before updating
||||||| last update
=======
# Keep the Node.js version in sync with .mise.toml.
>>>>>>> after updating
FROM node:24.18.0-slim AS base
WORKDIR /app
<<<<<<< before updating
RUN corepack enable
||||||| last update
=======
ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH
# Node.js 25+ no longer bundles Corepack: https://github.com/nodejs/corepack
RUN npm install -g corepack@0.35.0 && npm cache clean --force && corepack enable
>>>>>>> after updating

FROM base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
<<<<<<< before updating
RUN pnpm install --frozen-lockfile
||||||| last update
=======
RUN --mount=type=cache,id=pnpm-store,target=/pnpm/store \
    pnpm install --frozen-lockfile
>>>>>>> after updating

<<<<<<< before updating
# ローカル開発用ステージ (docker compose up agent で使用)
||||||| last update
=======
# Local development stage. Bind-mount the repo over /app (e.g. from
# docker compose, with an anonymous volume on /app/node_modules to keep
# this image's install instead of the host's) for live-reload without
# rebuilding the image.
>>>>>>> after updating
FROM base AS dev
COPY --from=deps /app/node_modules ./node_modules
CMD ["pnpm", "dev"]

FROM deps AS builder
<<<<<<< before updating
COPY . .
||||||| last update
=======
COPY tsconfig.json tsup.config.ts ./
COPY src ./src
>>>>>>> after updating
RUN pnpm run build

<<<<<<< before updating
# runAsUser/runAsGroup 1000 (node user) は infra 側 Deployment の securityContext で
# 強制される。kubelet は非数値の USER に対して runAsNonRoot を検証できないため、
# このイメージの node ユーザーの uid/gid 1000 を崩さないこと。
||||||| last update
=======
# Built fresh from `base`, not `builder`, so the runtime image doesn't inherit
# dev dependencies or source files left over from the build stage.
>>>>>>> after updating
FROM base AS runtime
ENV NODE_ENV=production
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
<<<<<<< before updating
RUN pnpm install --frozen-lockfile --prod
||||||| last update
=======
RUN --mount=type=cache,id=pnpm-store,target=/pnpm/store \
    pnpm install --frozen-lockfile --prod
>>>>>>> after updating
COPY --from=builder /app/dist ./dist
<<<<<<< before updating
COPY drizzle ./drizzle
EXPOSE 8080
||||||| last update
=======
>>>>>>> after updating
USER node
<<<<<<< before updating
CMD ["node", "dist/index.js"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
||||||| last update
=======
EXPOSE 8080
CMD ["node", "--import", "@fohte/service-kit/otel-register", "dist/index.js"]
>>>>>>> after updating
