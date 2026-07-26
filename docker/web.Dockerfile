# syntax=docker/dockerfile:1.7

FROM node:24-bookworm-slim AS builder

WORKDIR /workspace

ENV PNPM_HOME=/pnpm
ENV PATH="${PNPM_HOME}:${PATH}"

RUN corepack enable && corepack prepare pnpm@11.5.0 --activate

COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./web/
COPY web/app/package.json ./web/app/package.json
COPY web/packages/api-client/package.json ./web/packages/api-client/package.json
COPY web/packages/block-renderer/package.json ./web/packages/block-renderer/package.json
COPY web/packages/block-sdk/package.json ./web/packages/block-sdk/package.json
COPY web/packages/embed-sdk/package.json ./web/packages/embed-sdk/package.json
COPY web/packages/embedded-contracts/package.json ./web/packages/embedded-contracts/package.json
COPY web/packages/flow-schema/package.json ./web/packages/flow-schema/package.json
COPY web/packages/page-protocol/package.json ./web/packages/page-protocol/package.json
COPY web/packages/page-runtime/package.json ./web/packages/page-runtime/package.json
COPY web/packages/shared-types/package.json ./web/packages/shared-types/package.json
COPY web/packages/ui/package.json ./web/packages/ui/package.json

RUN --mount=type=cache,id=1flowbase-pnpm-store,sharing=locked,target=/pnpm/store \
    pnpm config set store-dir /pnpm/store \
    && pnpm config set package-import-method copy \
    && pnpm --dir web install --frozen-lockfile

COPY web ./web
COPY scripts ./scripts

RUN pnpm --dir web --filter @1flowbase/web build

FROM nginx:alpine AS runtime-base

COPY docker/web/nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

FROM runtime-base AS runtime-prebuilt

ARG TARGETARCH

COPY --from=web_dist /${TARGETARCH}/dist /usr/share/nginx/html

FROM runtime-base AS runtime

COPY --from=builder /workspace/web/app/dist /usr/share/nginx/html
