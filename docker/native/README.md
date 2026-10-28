# 1flowbase native portable distribution (experimental)

The Rust backend serves both the web UI and API and manages bundled PostgreSQL 18. Users do not need to install Docker, Nginx, Node.js or PostgreSQL. Third-party plugins still require artifacts for the target platform.

## Run

Extract the archive. On Windows, double-click `1flowbase.exe`. On Linux/macOS, run `./start.sh` as a regular user, without sudo. The browser opens `http://127.0.0.1:7800`; open this URL manually on a headless system. Keep the terminal open and press Ctrl+C to stop. This experimental version does not install a system service; closing the window or forcibly terminating the process is not a substitute for a graceful stop.

PostgreSQL listens only on `127.0.0.1:35432`. An occupied port causes an error; the launcher never adopts or stops another database. Platforms such as Windows ARM64 need matching PostgreSQL and plugin artifacts; an archive name does not certify platform support.

The default account is `root`. Its randomly generated initial password is the `root_password` field in `data/settings.json` beside the executable. Startup prints the file location. Keep this file private: it also contains the database password and credential encryption key.

## Persistent data

All platforms default to **`data/` beside the executable**, regardless of the working directory used to launch it. Extract the package to a writable directory; there is no automatic fallback to a user profile directory.

```text
1flowbase-windows-x64/
├── 1flowbase.exe
├── postgres/             # Bundled database binaries
└── data/                 # Created on first launch; preserve during upgrades
    ├── settings.json     # root_password, database password, encryption key
    ├── postgres/         # Database cluster
    ├── postgres.log
    ├── api/              # Business files, installed plugins, etc.
    └── backups/
```

Linux/macOS use the executable name `1flowbase` with the same data layout. Distribution archives never include existing `data/` or user passwords.

| Environment variable | Default | Purpose |
| --- | --- | --- |
| `FLOWBASE_DATA_DIR` | `<executable-directory>/data` | Explicit alternative data location; relative values resolve against the launch working directory |
| `FLOWBASE_PORT` | `7800` | Local web/API port |
| `FLOWBASE_POSTGRES_PORT` | `35432` | Private database port |
| `FLOWBASE_OPEN_BROWSER` | Enabled | Set to `false` to disable browser opening |

Example on Linux/macOS: `FLOWBASE_PORT=17800 FLOWBASE_POSTGRES_PORT=45432 ./start.sh`. On Windows PowerShell, set `$env:FLOWBASE_PORT='17800'`, then run `.\1flowbase.exe`.

Only one instance may use a data directory. Do not delete `settings.json` or copy/move live database files. Before upgrading, stop normally and create a restorable backup, then extract the new program archive while preserving the complete `data/` directory and configuration. When upgrading into another program directory, copy the complete `data/` there before launching. Automatic PostgreSQL major upgrades are not supported; schema migrations may prevent running an older application binary against the updated database. After an abnormal exit, if PostgreSQL is still running, verify the instance and stop it with the bundled `postgres/bin/pg_ctl -D <data-directory>/postgres stop -m fast -w` before restarting. Do not remove PID files manually.

## Migrating from earlier experimental packages

Earlier defaults were `%LOCALAPPDATA%\1flowbase` on Windows, `${XDG_DATA_HOME:-$HOME/.local/share}/1flowbase` on Linux, and `~/Library/Application Support/1flowbase` on macOS.

If the new local data directory has no configuration or database but the old location does, startup stops with both paths. It does not create an empty replacement database or move files automatically.

1. Stop the old application normally with Ctrl+C and confirm its PostgreSQL instance has stopped.
2. Copy the **entire contents** of the old directory into `data/` beside the new executable, including `settings.json`, `postgres/`, business files and plugins. Do not copy only the password file or merge two existing databases.
3. Launch the new package and confirm the original account and data work before deciding what to do with the retained old backup.

To intentionally keep using the old location, set `FLOWBASE_DATA_DIR` explicitly. Existing local data takes precedence; the launcher never merges or overwrites the old directory.

## Build and validation scope

Build `api-server` and `system_recovery` for each platform, build the web assets, and supply a complete relocatable PostgreSQL 18 distribution for that platform, including libraries, share files, licenses and the postgres, pg_ctl, initdb, psql, createdb, pg_dump and pg_restore tools. Node.js is needed only on the build machine.

```sh
cargo build --manifest-path api/Cargo.toml --release -p api-server --bin api-server --bin system_recovery
pnpm --dir web build
node scripts/node/native-release/resources.js tmp/native-resources
node scripts/node/native-release/fetch-postgres.js linux-x64 tmp/native-postgres
# Use the PostgreSQL directory printed by the command above.
node scripts/node/native-release/package.js \
  --platform linux --arch x64 \
  --api-binary api/target/release/api-server \
  --recovery-binary api/target/release/system_recovery \
  --web-dir web/app/dist --postgres-dir /path/to/postgresql \
  --resources-dir tmp/native-resources --out tmp/native-release
```

Platforms: `linux`, `macos`, `windows`. Architectures: `x64`, `arm64`. Example archive: `1flowbase-windows-x64.zip`. Version information belongs in the Release and `native-distribution.json`, never a hash suffix in the filename. Existing staging directories and archives are not overwritten.

Production releases should bundle existing model-pricing and application-template resources using `--resources-dir`, preserve PostgreSQL and bundled library license notices, and validate dependencies, functionality, backup/restore and signing/notarization on clean target systems. This experiment does not certify every plugin on all platforms and does not include automatic updates or system services.

On Windows, network-egress plugin cleanup currently terminates the direct child and reports descendant-tree cleanup as unverified. Do not rely on this capability for guaranteed descendant cleanup until Windows Job Object support is implemented.
