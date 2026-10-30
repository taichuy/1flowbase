'use strict';
const fs = require('node:fs');
const path = require('node:path');

function nativeEnvironment(repoRoot, platform) {
  const suffix = platform === 'windows' ? '.exe' : '';
  const overrides = new Map(Object.entries({
    API_ENV: 'production',
    API_SERVER_ADDR: '127.0.0.1:7800',
    API_DATABASE_URL: 'postgres://postgres:__GENERATE__@127.0.0.1:35432/1flowbase',
    API_AUTH_PUBLIC_ORIGIN: 'http://127.0.0.1:7800',
    API_ALLOWED_ORIGINS: 'http://127.0.0.1:7800,http://localhost:7800',
    API_COOKIE_SECURE: 'false',
    API_PROVIDER_SECRET_MASTER_KEY: '__GENERATE__',
    API_WORKSPACE_ROOT: 'api',
    API_BUSINESS_FILE_LOCAL_ROOT: 'data/api/storage',
    API_PROVIDER_INSTALL_ROOT: 'data/api/plugins',
    API_HOST_EXTENSION_DROPIN_ROOT: 'data/api/plugins/host-extension/dropins',
    API_MCP_TEMPLATE_LIBRARY_ROOT: 'data/api/storage/extension-center/mcp',
    API_SYSTEM_BACKUP_REPOSITORY_ROOT: 'data/backups',
    API_MODEL_PRICING_BOOTSTRAP_ROOT: 'api/resources/model-pricing',
    API_APPLICATION_TEMPLATE_ROOT: 'api/resources/application-templates',
    API_POSTGRES_PG_DUMP_PATH: `postgres/bin/pg_dump${suffix}`,
    API_POSTGRES_PG_RESTORE_PATH: `postgres/bin/pg_restore${suffix}`,
    FLOWBASE_DATA_DIR: 'data',
    FLOWBASE_OPEN_BROWSER: 'true',
  }));
  const source = fs.readFileSync(path.join(repoRoot, 'api/apps/api-server/.env.example'), 'utf8');
  const lines = ['# Native runtime configuration, generated from api/apps/api-server/.env.example.', '# Edit this file before launch. Paths are relative to the executable directory.', '# __GENERATE__ credentials are generated once and saved here; preserve this file with data/.', '# Literal dotenv values; no shell execution or variable expansion.'];
  const seen = new Set();
  for (const line of source.split(/\r?\n/)) {
    const match = line.match(/^([A-Z][A-Z0-9_]*)=(.*)$/);
    if (!match) continue; // Development-only commentary must not describe the portable profile.
    const [, key, value] = match;
    if (key.startsWith('VITE_')) continue;
    if (seen.has(key)) throw new Error(`Duplicate API example key: ${key}`);
    seen.add(key);
    lines.push(`${key}=${overrides.has(key) ? overrides.get(key) : value}`);
    overrides.delete(key);
  }
  for (const [key, value] of overrides) lines.push(`${key}=${value}`);
  for (const key of ['BOOTSTRAP_ROOT_ACCOUNT', 'BOOTSTRAP_ROOT_PASSWORD', 'BOOTSTRAP_ROOT_EMAIL', 'BOOTSTRAP_WORKSPACE_NAME']) {
    if (!seen.has(key)) throw new Error(`API example missing ${key}`);
  }
  return lines.join('\n') + '\n';
}

function maintenanceScripts(platform) {
  if (platform === 'windows') return {
    'upgrade.cmd': '@echo off\r\npowershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0upgrade.ps1"\r\n',
    'upgrade.ps1': `$ErrorActionPreference = 'Stop'
$installation = $PSScriptRoot
$worker = Join-Path ([System.IO.Path]::GetTempPath()) ('1flowbase-upgrade-' + [guid]::NewGuid())
$result = 1
try {
  New-Item -ItemType Directory -Path $worker | Out-Null
  $executable = Join-Path $worker '1flowbase.exe'
  Copy-Item -LiteralPath (Join-Path $installation '1flowbase.exe') -Destination $executable
  & $executable --upgrade $installation
  $result = $LASTEXITCODE
} catch { Write-Host $_ -ForegroundColor Red }
finally {
  Remove-Item -LiteralPath $worker -Recurse -Force -ErrorAction SilentlyContinue
}
Read-Host 'Press Enter to close'
exit $result
`.replaceAll('\n', '\r\n'),
  };
  return { 'upgrade.sh': `#!/bin/sh
set -eu
package_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
worker=$(mktemp -d "\${TMPDIR:-/tmp}/1flowbase-upgrade.XXXXXXXX")
trap 'rm -rf -- "$worker"' EXIT HUP INT TERM
cp -- "$package_dir/1flowbase" "$worker/1flowbase"
"$worker/1flowbase" --upgrade "$package_dir"
` };
}
module.exports = { nativeEnvironment, maintenanceScripts };
