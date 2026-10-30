# 1flowbase portable distribution

The package includes the Rust backend, web assets, PostgreSQL 18 and an updater. Neither startup nor updates require Docker, Nginx, Node.js or Python.

## Start and configure

Download the archive for your OS and architecture from GitHub Releases and extract it once into a writable directory. On Windows, double-click `1flowbase.exe`. On Linux/macOS, run `./start.sh` as a regular user. The default browser opens `http://127.0.0.1:7800`. Keep the terminal open; press Ctrl+C to stop and wait for PostgreSQL to shut down.

The root `.env` is the runtime configuration, generated from `api/apps/api-server/.env.example` with portable ports, production mode and paths. The default account is `root`, with initial password `root@1flowbase123`. Edit `BOOTSTRAP_ROOT_*` before the first launch to change those defaults. Restarting does not reset existing accounts.

```text
1flowbase-windows-x64/
├── 1flowbase.exe
├── .env                      # User configuration, preserved during updates
├── .env.example              # Defaults for this version
├── upgrade.cmd               # Double-click to update; no directory prompt
├── upgrade.ps1               # Windows update launcher
├── native-distribution.json  # Version, platform and file inventory
├── api/  web/  postgres/     # Program and resources
└── data/                     # Database, business files and installed plugins
```

Linux/macOS use `1flowbase`, `start.sh` and `upgrade.sh`. Archive names contain only the product, OS and architecture; versions belong to the Release.

- Configuration is always read beside the executable. Relative paths resolve from that directory. File entries override matching inherited environment variables.
- Use UTF-8 `KEY=value` entries, single-quoted literals, JSON-style double-quoted escapes and comments. No shell execution or variable expansion occurs. Duplicate keys and malformed entries fail explicitly.
- `__GENERATE__` database passwords and encryption keys are generated and saved to `.env` on first launch, then remain stable. Preserve the original `.env` with your data. Do not arbitrarily change credentials for an existing database or its encryption key.
- `API_SERVER_ADDR` defaults to `127.0.0.1:7800`. `API_DATABASE_URL` points to bundled PostgreSQL on `127.0.0.1:35432`, using the `postgres` user and `1flowbase` database.
- If changing the public address, update `API_AUTH_PUBLIC_ORIGIN` / `API_ALLOWED_ORIGINS` as well. If changing `FLOWBASE_DATA_DIR`, adjust the related storage paths.
- Set `FLOWBASE_OPEN_BROWSER=false` to suppress browser opening. Restart after configuration changes.

## Update this installation

1. Press Ctrl+C to stop the application and wait for PostgreSQL to stop normally.
2. In the **current installation**, double-click `upgrade.cmd`, or run `./upgrade.sh` on Linux/macOS.
3. Wait for download, verification and replacement to finish, then start normally.

You do not need to choose an architecture, enter an installation path or download the new archive yourself. The launcher locates the installation from its own path. Windows uses built-in PowerShell and runs the updater from a temporary directory so its executable can be replaced.

The updater queries the official `taichuy/1flowbase` GitHub Releases for a higher version with the matching native asset. Stable installations only accept stable releases; prerelease installations may accept higher prereleases or stable versions. If already current, it reports that and exits. Network errors stop the operation before replacement.

The download's official SHA256, archive paths and file inventory are verified, followed by platform, architecture, PostgreSQL major, space, configuration and stopped-process checks. The updater then **replaces program files in the current installation, preserving `.env` and user data. It creates no backup and provides no rollback.** It runs initialization, migrations and a health check. `.env.example` becomes the new reference; your `.env` is not automatically merged.

Downloaded files are staged beside the installation in `.<installation-name>-update/`. This holds only new program files and update logs; it never copies the old program, configuration or database. It is removed after success.

## Failed or interrupted updates

Download or verification failures leave the current program untouched. Once replacement starts, `.upgrade-state.json` records incomplete work and blocks normal startup. Confirm the application and PostgreSQL are stopped, then **run the same update launcher again**. It downloads a newer corrective release if available, otherwise retries the verified pending package. Keep the pending package and state file intact.

Initialization errors are recorded in `.<installation-name>-update/startup-validation.log`. Correct configuration errors and retry. A defective release requires a corrective release. With no backup, completed migrations and data changes are not rolled back.

Updates require the same OS, architecture and PostgreSQL major version. PostgreSQL major upgrades need a separate migration. Occupied ports, damaged packages and running processes stop the update; unrelated databases are never adopted or stopped.

This version introduces the online launcher; older scripts do not acquire this behavior automatically. Download this package to start using it. Early experimental packages without a root `.env` are not an upgrade baseline.

## Build and verification boundaries

Official releases follow `container images`. Use `node scripts/node/cli/bump-version.js` to synchronize versions, then merge into `latest` through the existing release process. Docker images and all five native targets build automatically. `VERSION`, frontend and backend versions must match. After both distribution types pass their build/validation jobs, a single stable GitHub Release contains the Docker instructions and native downloads at the same version.

`native-build.yml` is the shared builder and retains actual-archive, initialization, editable-configuration, no-backup update, retry and launcher checks. `Native portable distribution` remains available for manual validation and independent prereleases. Ordinary source changes without a version change do not create another release. Re-running an already complete release at the same source commit skips publication rather than replacing public assets.

`system_recovery` remains a separate business backup/recovery tool and is not invoked by the updater. Signing/notarization, system services and unattended background updates are outside this feature's scope. Existing Windows network-egress descendant-process cleanup limitations still apply.
