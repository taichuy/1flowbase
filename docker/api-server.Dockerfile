# syntax=docker/dockerfile:1.7

FROM rust:1-slim-bookworm AS builder

ARG TARGETARCH
ARG TARGETOS

WORKDIR /workspace/api

RUN apt-get update \
  && apt-get install -y --no-install-recommends build-essential ca-certificates curl pkg-config \
  && rm -rf /var/lib/apt/lists/*

COPY api/Cargo.toml api/Cargo.lock ./
COPY api/apps ./apps
COPY api/crates ./crates

RUN --mount=type=cache,id=1flowbase-cargo-registry,sharing=locked,target=/usr/local/cargo/registry \
    --mount=type=cache,id=1flowbase-cargo-git,sharing=locked,target=/usr/local/cargo/git \
    --mount=type=cache,id=1flowbase-rust-target-${TARGETOS}-${TARGETARCH},sharing=locked,target=/workspace/api/target-cache \
    CARGO_TARGET_DIR=/workspace/api/target-cache \
      cargo build --release -p api-server --bin api-server \
    && cp /workspace/api/target-cache/release/api-server /workspace/api/api-server

FROM alpine:3.22 AS default-extension

ARG TARGETARCH

RUN apk add --no-cache ca-certificates curl jq

COPY api/plugins/default-extensions.lock.json /tmp/default-extensions.lock.json
COPY scripts/shell/package-default-extension.sh /usr/local/bin/package-default-extension

RUN package-default-extension /tmp/default-extensions.lock.json "${TARGETARCH}" /default-extensions

FROM debian:trixie-slim AS runtime-base

ARG APP_UID=1000
ARG APP_GID=1000

WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && groupadd --gid "${APP_GID}" flowbase \
  && useradd --uid "${APP_UID}" --gid "${APP_GID}" --create-home --shell /usr/sbin/nologin flowbase

COPY api/plugins /app/api/plugins
COPY --from=default-extension /default-extensions /app/api/plugins/bootstrap

RUN mkdir -p \
    /app/api/storage \
    /app/api/plugins/packages \
    /app/api/plugins/installed \
    /app/api/plugins/host-extension/dropins \
  && chown -R flowbase:flowbase /app

USER flowbase

EXPOSE 7800

ENTRYPOINT ["/usr/local/bin/api-server"]

FROM runtime-base AS runtime

COPY --from=builder /workspace/api/api-server /usr/local/bin/api-server

FROM runtime-base AS runtime-prebuilt

ARG TARGETARCH

COPY --from=api_server_binaries /${TARGETARCH}/api-server /usr/local/bin/api-server
