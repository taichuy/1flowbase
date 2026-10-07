import { expect, test } from 'vitest';
import type { ConsoleApplicationCollector } from '@1flowbase/api-client';
import { collectorInstallerCommand } from '../../components/collector/installer-command';
const collector = {
  shell_installer_url: 'https://example.com/install.sh',
  powershell_installer_url: 'https://example.com/install.ps1',
  version: '0.1.0'
} as ConsoleApplicationCollector;
test('shell and PowerShell commands quote public parameters and clean temporary scripts', () => {
  const shell = collectorInstallerCommand(
    collector,
    'shell',
    "https://example.com/o'h/api/logs/v1/events",
    'application-one'
  );
  expect(shell).toContain("o'\\''h");
  expect(shell).toContain('trap');
  expect(shell).toContain(' -o "$installer" && bash "$installer"');
  const powershell = collectorInstallerCommand(
    collector,
    'powershell',
    "https://example.com/o'h/api/logs/v1/events",
    'application-two'
  );
  expect(powershell).toContain("o''h");
  expect(powershell).toContain('Remove-Item -LiteralPath');
  expect(powershell).toContain("-InstallationId 'application-two'");
  expect(shell + powershell).not.toMatch(/api_key|API_KEY|node scripts|npx/);
});
