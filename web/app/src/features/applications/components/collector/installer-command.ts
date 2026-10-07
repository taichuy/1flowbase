import type { ConsoleApplicationCollector } from '@1flowbase/api-client';

export type CollectorOperatingSystem = 'shell' | 'powershell';
const shellQuote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
const powershellQuote = (value: string) => `'${value.replaceAll("'", "''")}'`;

export function collectorInstallerCommand(
  collector: ConsoleApplicationCollector,
  operatingSystem: CollectorOperatingSystem,
  endpoint: string,
  applicationId: string
): string {
  if (operatingSystem === 'powershell') {
    return `& { $ErrorActionPreference = 'Stop'; $installer = Join-Path ([System.IO.Path]::GetTempPath()) (([guid]::NewGuid().ToString()) + '.ps1'); try { Invoke-WebRequest -UseBasicParsing -Uri ${powershellQuote(collector.powershell_installer_url)} -OutFile $installer; & $installer -Endpoint ${powershellQuote(endpoint)} -Version ${powershellQuote(collector.version)} -InstallationId ${powershellQuote(applicationId)} } finally { Remove-Item -LiteralPath $installer -ErrorAction SilentlyContinue } }`;
  }
  return `(installer="$(mktemp)" && trap 'rm -f "$installer"' EXIT && curl -fsSL ${shellQuote(collector.shell_installer_url)} -o "$installer" && bash "$installer" --endpoint ${shellQuote(endpoint)} --version ${shellQuote(collector.version)} --installation-id ${shellQuote(applicationId)})`;
}
