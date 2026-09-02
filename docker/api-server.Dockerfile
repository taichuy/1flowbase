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
      cargo build --release -p api-server --bin api-server --bin system_recovery \
    && cp /workspace/api/target-cache/release/api-server /workspace/api/api-server \
    && cp /workspace/api/target-cache/release/system_recovery /workspace/api/system_recovery

FROM alpine:3.22 AS default-extension

ARG TARGETARCH

RUN apk add --no-cache ca-certificates curl jq

COPY api/plugins/default-extensions.lock.json /tmp/default-extensions.lock.json
COPY scripts/shell/package-default-extension.sh /usr/local/bin/package-default-extension

RUN package-default-extension /tmp/default-extensions.lock.json "${TARGETARCH}" /default-extensions

FROM alpine:3.22 AS model-pricing-bootstrap

ARG MODEL_PRICING_REPOSITORY=taichuy/1flowbase-official-plugins
ARG MODEL_PRICING_REF=main

RUN apk add --no-cache ca-certificates git jq

COPY scripts/shell/package-model-pricing-bootstrap.sh /usr/local/bin/package-model-pricing-bootstrap

RUN chmod 0755 /usr/local/bin/package-model-pricing-bootstrap \
  && package-model-pricing-bootstrap \
      "${MODEL_PRICING_REPOSITORY}" \
      "${MODEL_PRICING_REF}" \
      /model-pricing

FROM node:24-bookworm-slim AS runtime-base

ARG APP_UID=1000
ARG APP_GID=1000

WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl gnupg \
  && install -d -m 0755 /usr/share/postgresql-common/pgdg \
  && curl -fsSL https://www.postgresql.org/media/keys/ACCC4CF8.asc \
    | gpg --dearmor -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.gpg \
  && echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.gpg] https://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" \
    > /etc/apt/sources.list.d/pgdg.list \
  && apt-get update \
  && apt-get install -y --no-install-recommends postgresql-client-18 \
  && /usr/lib/postgresql/18/bin/pg_dump --version | grep -Eq 'PostgreSQL\) 18[.]' \
  && /usr/lib/postgresql/18/bin/pg_restore --version | grep -Eq 'PostgreSQL\) 18[.]' \
  && apt-get purge -y --auto-remove curl gnupg \
  && rm -rf /var/lib/apt/lists/* \
  && groupmod --gid "${APP_GID}" --new-name flowbase node \
  && usermod --uid "${APP_UID}" --gid "${APP_GID}" --login flowbase \
    --home /home/flowbase --move-home --shell /usr/sbin/nologin node

ENV API_POSTGRES_PG_DUMP_PATH=/usr/lib/postgresql/18/bin/pg_dump \
    API_POSTGRES_PG_RESTORE_PATH=/usr/lib/postgresql/18/bin/pg_restore \
    API_MODEL_PRICING_BOOTSTRAP_ROOT=/app/api/resources/model-pricing

COPY api/plugins /app/api/plugins
COPY --from=default-extension /default-extensions /app/api/plugins/bootstrap
COPY --from=model-pricing-bootstrap /model-pricing /app/api/resources/model-pricing
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
COPY --from=builder /workspace/api/system_recovery /usr/local/bin/system_recovery

FROM runtime-base AS runtime-prebuilt

ARG TARGETARCH

COPY --from=api_server_binaries /${TARGETARCH}/api-server /usr/local/bin/api-server
COPY --from=api_server_binaries /${TARGETARCH}/system_recovery /usr/local/bin/system_recovery
