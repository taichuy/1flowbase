import { expect, test } from 'vitest';
import type { ConsoleApplicationCollector } from '@1flowbase/api-client';
import { collectorInstallerCommand } from '../../components/collector/installer-command';
const collector = {
  installation_status: 'installed',
  shell_installer_url: '/assets/install.sh',
  powershell_installer_url: '/assets/install.ps1',
  asset_base_url: '/assets',
  installed_version: '0.1.0',
  version: '0.2.0'
} as ConsoleApplicationCollector;
test('both commands pin installed version and current platform assets and quote parameters', () => {
  const shell = collectorInstallerCommand(
    collector,
    'shell',
    "https://example.com/o'h/api/logs/v1/events",
    'application-one',
    'https://example.com'
  )!;
  expect(shell).toContain("o'\\''h");
  expect(shell).toContain('trap');
  expect(shell).toContain(' -o "$installer" && bash "$installer"');
  expect(shell).toContain("--release-base 'https://example.com/assets'");
  expect(shell).toContain("--version '0.1.0'");
  const powershell = collectorInstallerCommand(
    collector,
    'powershell',
    "https://example.com/o'h/api/logs/v1/events",
    'application-two',
    'https://example.com'
  )!;
  expect(powershell).toContain("o''h");
  expect(powershell).toContain('Invoke-WebRequest -UseBasicParsing');
  expect(powershell).toContain(
    'powershell.exe -NoProfile -ExecutionPolicy Bypass -File $installer'
  );
  expect(powershell).toContain('if ($LASTEXITCODE -ne 0)');
  expect(powershell).toContain('Remove-Item -LiteralPath');
  expect(powershell).toContain("-ReleaseBase 'https://example.com/assets'");
  expect(powershell).toContain("-Version '0.1.0'");
  expect(shell + powershell).not.toMatch(
    /0\.2\.0|github|api_key|API_KEY|node scripts|npx/
  );
});
test.each(['not_installed', 'missing'] as const)(
  'unusable platform state %s cannot generate a command',
  (installation_status) => {
    expect(
      collectorInstallerCommand(
        { ...collector, installation_status },
        'shell',
        'https://example.com/api/logs/v1/events',
        'application',
        'https://example.com'
      )
    ).toBeNull();
  }
);
test('missing local URL cannot generate even with installed metadata', () => {
  expect(
    collectorInstallerCommand(
      { ...collector, asset_base_url: null },
      'powershell',
      'https://example.com/api/logs/v1/events',
      'application',
      'https://example.com'
    )
  ).toBeNull();
});
