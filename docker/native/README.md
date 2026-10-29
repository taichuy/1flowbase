# 1flowbase native portable distribution

The package includes the Rust API/static-web host, PostgreSQL 18 and offline maintenance tools. Running or upgrading it requires no Docker, Nginx, Node.js or Python installation.

## First launch and configuration

Download the archive for your OS/architecture from GitHub Releases and extract it once into a writable directory. On Windows, launch `1flowbase.exe`; on Linux/macOS, run `./start.sh` as a regular user. The default browser opens `http://127.0.0.1:7800`. Keep the terminal open; use Ctrl+C and wait for PostgreSQL to stop normally.

The root `.env` is the runtime configuration. It is generated from `api/apps/api-server/.env.example` with native production-mode, localhost and resource-path adjustments. Defaults are account `root` and initial password `root@1flowbase123`; edit `BOOTSTRAP_ROOT_*` before first launch if desired. Restarting does not reset existing accounts.

```text
1flowbase-windows-x64/
├── 1flowbase.exe
├── .env                      # User configuration, preserved during upgrades
├── .env.example              # This release's defaults, for comparison
├── upgrade.cmd               # Windows upgrade/restore entry point
├── native-distribution.json  # Version, platform, PG major and file checksums
├── api/  web/  postgres/     # Managed program files and resources
└── data/                     # Created on first launch; preserved during upgrades
    ├── postgres/
    ├── postgres.log
    ├── api/                  # Business files and installed plugins
    └── backups/
```

Linux/macOS use `1flowbase`, `upgrade.sh` and `start.sh`. Archive names contain the product, OS and architecture; version and checksum information belongs to the Release/manifest.

Configuration rules:

- The launcher reads `.env` beside its executable regardless of the terminal's working directory. Relative paths use that same base.
- Entries in `.env` override inherited environment variables with the same names. The launcher no longer reads `settings.json` or embedded Docker defaults.
- Use UTF-8. Supported syntax includes `KEY=value`, single-quoted literals, JSON-style double-quoted escapes, and comments. There is no shell execution or variable expansion. Duplicate keys and malformed values are errors.
- `__GENERATE__` placeholders for the database password and encryption key are generated once and saved back to `.env`. Preserve this file with the complete data directory. Secrets are never regenerated for an existing cluster.
- Restart after editing configuration. A database password must match the existing PostgreSQL credential; retain the encryption key used by your data.

| Setting | Native default / purpose |
| --- | --- |
| `API_SERVER_ADDR` | `127.0.0.1:7800`, web/API address |
| `API_DATABASE_URL` | Authoritative private PG connection settings; port `35432` |
| `API_AUTH_PUBLIC_ORIGIN` / `API_ALLOWED_ORIGINS` | Default to the 7800 origin; update together with your access address |
| `FLOWBASE_DATA_DIR` | `data`, database and instance data |
| `API_BUSINESS_FILE_LOCAL_ROOT` and other storage paths | Under `data/` by default; update dependent paths when relocating data |
| `FLOWBASE_OPEN_BROWSER` | `true`; set `false` to disable automatic browser opening |

This entry point manages the local `postgres` user and `1flowbase` database only. It does not adopt external databases or stop other instances. Port conflicts are reported explicitly.

## Offline upgrade

This layout establishes the supported upgrade baseline. Earlier experimental packages without a root `.env` or versioned manifest are not supported upgrade sources; no experimental password/directory migration is included.

1. Stop the old application normally with Ctrl+C and wait for PostgreSQL to stop.
2. Extract the new archive **outside** the existing installation. Do not overwrite it manually.
3. Run the upgrade entry point from the new package and select the existing installation. Double-clicking `upgrade.cmd` prompts for the directory, or use:

```powershell
.\upgrade.cmd "E:\Apps\1flowbase"
```

Linux/macOS:

```sh
./upgrade.sh /path/to/existing/1flowbase
```

The updater verifies checksums, OS/architecture, versions, PG major, configuration, free space and stopped processes. It creates a complete cold snapshot, replaces managed program files, preserves `.env` and user data, then runs application initialization, migrations, health checks and shutdown validation. Validation does not expose business HTTP traffic or open a browser. After success, start the application normally from the **original installation directory**.

Snapshots are stored next to that directory in `.<directory-name>-upgrade-backups/<backup-id>/`, containing the old program, `.env`, complete data, checksummed receipt and validation log. They are never deleted automatically. `.env.example` is updated; user `.env` is never merged or overwritten. Missing required configuration is reported before replacement so you can compare and amend it.

Only upgrades on the same OS, architecture and PostgreSQL major version are supported. PG major upgrades require a separate migration. Data must be in `data/` or a separate external directory; business-file, plugin and backup paths must all reside within the configured data directory. Symlinks and external tablespaces are outside this cold-snapshot contract and cause a refusal.

## Failure and restoration

Validation or snapshot failure before replacement leaves the existing program and data unchanged. Once replacement begins, `.upgrade-state.json` records the snapshot and phase. Failure or interruption blocks normal launch and retains recovery information.

After confirming all related processes and PostgreSQL have stopped, run the explicit restore command from the **new package**:

```powershell
.\upgrade.cmd --restore "E:\Apps\1flowbase" "E:\Apps\.1flowbase-upgrade-backups\backup-id"
```

```sh
./upgrade.sh --restore /path/to/existing/1flowbase /path/to/.1flowbase-upgrade-backups/backup-id
```

Restore verifies snapshot ownership and checksums, then restores the matching **program, configuration and complete data**. Repeat the same command to resume an interrupted restore. It discards changes made after the snapshot; replacing only the executable is not a valid rollback after database migrations. Corrupt backups, mismatched targets and active processes are rejected before restoration.

See `startup-validation.log` inside the snapshot directory. Do not delete PostgreSQL's `postmaster.pid`. If an abnormal exit leaves the owned PG instance running, verify it and stop it with the bundled `postgres/bin/pg_ctl -D <data-directory>/postgres stop -m fast -w`.

## Build and evidence boundaries

The `Native portable distribution` workflow builds all native artifacts and exercises configuration/upgrade negative cases, real PostgreSQL lifecycle and real-package upgrade/restoration. With `release_tag` and target `all`, single-layer archives are uploaded to GitHub Releases only after every platform succeeds.

`system_recovery` remains the system-backup recovery utility; it is not this program updater. Plugin platform availability, signing/notarization, system services and online automatic updates are outside this offline-upgrade feature. Windows network-egress descendant-process cleanup retains its existing support boundary.
