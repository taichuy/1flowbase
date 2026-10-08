import type { ConsoleApplicationCollector } from '@1flowbase/api-client';

export type CollectorOperatingSystem = 'shell' | 'powershell';
const shellQuote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
const powershellQuote = (value: string) => `'${value.replaceAll("'", "''")}'`;

export function collectorInstallerCommand(
  collector: ConsoleApplicationCollector,
  operatingSystem: CollectorOperatingSystem,
  endpoint: string,
  applicationId: string,
  apiBaseUrl: string,
  apiKey = ''
): string | null {
  if (
    collector.installation_status !== 'installed' ||
    !collector.installed_version ||
    !collector.asset_base_url ||
    !collector.shell_installer_url ||
    !collector.powershell_installer_url
  )
    return null;
  const downloadUrl = (path: string) =>
    `${apiBaseUrl.replace(/\/$/, '')}${path}`;
  const releaseBase = downloadUrl(collector.asset_base_url);
  if (operatingSystem === 'powershell') {
    const saveKey = apiKey
      ? '$previousCollectorKey = $env:FLOWBASE_AGENT_LOGS_API_KEY; '
      : '';
    const setKey = apiKey
      ? `$env:FLOWBASE_AGENT_LOGS_API_KEY = ${powershellQuote(apiKey)}; `
      : '';
    const restoreKey = apiKey
      ? '$env:FLOWBASE_AGENT_LOGS_API_KEY = $previousCollectorKey; '
      : '';
    return `& { $ErrorActionPreference = 'Stop'; ${saveKey}$installer = Join-Path ([System.IO.Path]::GetTempPath()) (([guid]::NewGuid().ToString()) + '.ps1'); try { Invoke-WebRequest -UseBasicParsing -MaximumRedirection 0 -Uri ${powershellQuote(downloadUrl(collector.powershell_installer_url))} -OutFile $installer; ${setKey}powershell.exe -NoProfile -ExecutionPolicy Bypass -File $installer -Endpoint ${powershellQuote(endpoint)} -Version ${powershellQuote(collector.installed_version)} -ReleaseBase ${powershellQuote(releaseBase)} -InstallationId ${powershellQuote(applicationId)}; if ($LASTEXITCODE -ne 0) { throw "Collector installation failed (exit $LASTEXITCODE)" } } finally { ${restoreKey}Remove-Item -LiteralPath $installer -ErrorAction SilentlyContinue } }`;
  }
  const setKey = apiKey
    ? `FLOWBASE_AGENT_LOGS_API_KEY=${shellQuote(apiKey)} `
    : '';
  return `(installer="$(mktemp)" && trap 'rm -f "$installer"' EXIT && curl -fsSL --max-redirs 0 ${shellQuote(downloadUrl(collector.shell_installer_url))} -o "$installer" && ${setKey}bash "$installer" --endpoint ${shellQuote(endpoint)} --version ${shellQuote(collector.installed_version)} --release-base ${shellQuote(releaseBase)} --installation-id ${shellQuote(applicationId)})`;
}
