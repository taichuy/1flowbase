import type { ConsoleApplicationCollector } from '@1flowbase/api-client';

export type CollectorOperatingSystem = 'shell' | 'powershell';
const shellQuote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
const powershellQuote = (value: string) => `'${value.replaceAll("'", "''")}'`;

export function collectorInstallerCommand(
  collector: ConsoleApplicationCollector,
  operatingSystem: CollectorOperatingSystem,
  endpoint: string,
  applicationId: string,
  apiBaseUrl: string
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
    return `& { $ErrorActionPreference = 'Stop'; $installer = Join-Path ([System.IO.Path]::GetTempPath()) (([guid]::NewGuid().ToString()) + '.ps1'); try { Invoke-WebRequest -UseBasicParsing -Uri ${powershellQuote(downloadUrl(collector.powershell_installer_url))} -OutFile $installer; powershell.exe -NoProfile -ExecutionPolicy Bypass -File $installer -Endpoint ${powershellQuote(endpoint)} -Version ${powershellQuote(collector.installed_version)} -ReleaseBase ${powershellQuote(releaseBase)} -InstallationId ${powershellQuote(applicationId)}; if ($LASTEXITCODE -ne 0) { throw "Collector installation failed (exit $LASTEXITCODE)" } } finally { Remove-Item -LiteralPath $installer -ErrorAction SilentlyContinue } }`;
  }
  return `(installer="$(mktemp)" && trap 'rm -f "$installer"' EXIT && curl -fsSL ${shellQuote(downloadUrl(collector.shell_installer_url))} -o "$installer" && bash "$installer" --endpoint ${shellQuote(endpoint)} --version ${shellQuote(collector.installed_version)} --release-base ${shellQuote(releaseBase)} --installation-id ${shellQuote(applicationId)})`;
}
