# Single-file Docker deployment

[简体中文](README_CN.md)

Save `compose.yaml` in an empty directory and run:

```bash
docker compose up -d
```

Open `http://localhost:3100`. The initial account is `root` and the initial password is
`change-me-root-password`; change it after signing in. This recipe requires an API
image containing `/usr/local/lib/1flowbase/initialize.cjs` and
`/usr/local/bin/1flowbase-api-start`. Images published before this recipe do not contain them.

The initialization service runs a script built into the API image. It creates mounted
directories and `config/.env`, generates a separate database password and encryption
master key, then exits. PostgreSQL and the API read the saved configuration.
Recreating containers preserves it. Keep the configuration together with the database
and uploaded files; an existing database without its configuration fails to start.

Before first startup, an optional Compose `.env` next to `compose.yaml` can set
`WEB_PORT`, `FLOWBASE_API_SERVER_VERSION`, `FLOWBASE_WEB_VERSION`,
`BOOTSTRAP_ROOT_ACCOUNT`, `BOOTSTRAP_ROOT_PASSWORD`, `POSTGRES_PASSWORD`, and
`API_PROVIDER_SECRET_MASTER_KEY`. Initialization values apply only when creating
`config/.env`; changing them does not reset existing accounts or rotate existing secrets.
The default is HTTP. Set `API_COOKIE_SECURE=true` for a browser-facing HTTPS entry,
including TLS terminated by an outer nginx proxy. Set `API_ALLOWED_ORIGINS` when
browser clients access the API from a different origin.

`config/.env` contains secrets. Do not commit it or delete it to reset an account.
This recipe is for a fresh deployment with bundled PostgreSQL. Existing deployments,
external PostgreSQL, and portable recovery retain the deployment scripts and recipes
documented in [the Docker guide](../README.md).
