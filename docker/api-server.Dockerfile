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
      cargo build --release -p api-server --bin api-server --bin system_recovery --bin frontstage_executable_upgrade \
    && cp /workspace/api/target-cache/release/api-server /workspace/api/api-server \
    && cp /workspace/api/target-cache/release/system_recovery /workspace/api/system_recovery \
    && cp /workspace/api/target-cache/release/frontstage_executable_upgrade /workspace/api/frontstage_executable_upgrade

FROM node:24-bookworm-slim AS frontstage-executable-compiler

WORKDIR /workspace/web

RUN npm install --global pnpm@11.5.0

COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./
COPY web/packages ./packages

RUN pnpm install --frozen-lockfile --prod --filter @1flowbase/tailwindcss-catalog... \
  && test "$(sha256sum packages/tailwindcss-catalog/bin/compiler-4.3.3.mjs | cut -d' ' -f1)" = "603eb3ed18b81b7de3ce3f0e1f6f599dc1c6d58e246b6f567bad59e2a4d0a704" \
  && node packages/tailwindcss-catalog/bin/compiler-4.3.3.mjs <<'EOF' | grep -q '"artifact_sha256":"db8e4ecacf25ed2a926cbd5e8dfb4d5abeaf9db6bfe7025cd5a8fdaabed7efaf"'
{"source_code":"export default () => null;","dependency_lock":[],"compiler_identity":{"name":"@1flowbase/tailwindcss-catalog","contract":"source-driven-utilities-v1","tailwind_version":"4.3.3"},"toolchain_lock":{"package":"tailwindcss","version":"4.3.3","mode":"theme-and-utilities"}}
EOF

FROM alpine:3.22 AS default-extension

ARG TARGETARCH

RUN apk add --no-cache ca-certificates curl jq

COPY api/plugins/default-extensions.lock.json /tmp/default-extensions.lock.json
COPY scripts/shell/package-default-extension.sh /usr/local/bin/package-default-extension

RUN package-default-extension /tmp/default-extensions.lock.json "${TARGETARCH}" /default-extensions

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
  && groupadd --gid "${APP_GID}" flowbase \
  && useradd --uid "${APP_UID}" --gid "${APP_GID}" --create-home --shell /usr/sbin/nologin flowbase

ENV API_POSTGRES_PG_DUMP_PATH=/usr/lib/postgresql/18/bin/pg_dump \
    API_POSTGRES_PG_RESTORE_PATH=/usr/lib/postgresql/18/bin/pg_restore

COPY api/plugins /app/api/plugins
COPY --from=default-extension /default-extensions /app/api/plugins/bootstrap
COPY --from=frontstage-executable-compiler /workspace/web/node_modules /app/frontstage-executable-compiler/node_modules
COPY --from=frontstage-executable-compiler /workspace/web/packages/tailwindcss-catalog /app/frontstage-executable-compiler/packages/tailwindcss-catalog
COPY --from=frontstage-executable-compiler /workspace/web/packages/page-runtime /app/frontstage-executable-compiler/packages/page-runtime
COPY --from=frontstage-executable-compiler /workspace/web/packages/page-protocol /app/frontstage-executable-compiler/packages/page-protocol

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
COPY --from=builder /workspace/api/frontstage_executable_upgrade /usr/local/bin/frontstage_executable_upgrade

FROM runtime-base AS runtime-prebuilt

ARG TARGETARCH

COPY --from=api_server_binaries /${TARGETARCH}/api-server /usr/local/bin/api-server
COPY --from=api_server_binaries /${TARGETARCH}/system_recovery /usr/local/bin/system_recovery
COPY --from=api_server_binaries /${TARGETARCH}/frontstage_executable_upgrade /usr/local/bin/frontstage_executable_upgrade
