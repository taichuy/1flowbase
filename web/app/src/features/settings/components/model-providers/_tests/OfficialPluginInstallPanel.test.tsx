import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type { SettingsOfficialPluginCatalogEntry } from '../../../api/plugins';
import { OfficialPluginInstallPanel } from '../OfficialPluginInstallPanel';

type OfficialPluginInstallPanelProps = Parameters<
  typeof OfficialPluginInstallPanel
>[0];

const baseEntry: SettingsOfficialPluginCatalogEntry = {
  plugin_id: '1flowbase.openai_compatible',
  provider_code: 'openai_compatible',
  plugin_type: 'model_provider',
  display_name: 'OpenAI Compatible',
  description: '面向 OpenAI 兼容 Chat Completions API 的 provider 插件。',
  protocol: 'openai_compatible',
  latest_version: '0.3.17',
  selected_artifact: {
    os: 'linux',
    arch: 'amd64',
    libc: 'musl',
    rust_target: 'x86_64-unknown-linux-musl',
    download_url: 'https://example.com/openai-compatible.1flowbasepkg',
    checksum: 'sha256:abc123',
    signature_algorithm: null,
    signing_key_id: null
  },
  help_url: 'https://platform.openai.com/docs/api-reference',
  model_discovery_mode: 'hybrid',
  install_status: 'not_installed',
  minimum_host_version: '0.1.0',
  current_host_version: '0.2.0',
  compatibility_status: 'compatible',
  compatibility_warning_reason: null
};

function renderPanel(
  entry: SettingsOfficialPluginCatalogEntry,
  handlers: Partial<
    Pick<OfficialPluginInstallPanelProps, 'onInstall' | 'onUpgradeLatest'>
  > = {},
  sourceMeta: OfficialPluginInstallPanelProps['sourceMeta'] = null
) {
  const onInstall =
    handlers.onInstall ?? vi.fn<OfficialPluginInstallPanelProps['onInstall']>();
  const onUpgradeLatest =
    handlers.onUpgradeLatest ??
    vi.fn<OfficialPluginInstallPanelProps['onUpgradeLatest']>();

  render(
    <OfficialPluginInstallPanel
      sourceMeta={sourceMeta}
      entries={[entry]}
      familiesByProviderCode={{}}
      canManage
      searchQuery=""
      activePluginId={null}
      installState="idle"
      upgradingProviderCode={null}
      onInstall={onInstall}
      onOpenUpload={vi.fn()}
      onSearchQueryChange={vi.fn()}
      onRetryCatalog={vi.fn()}
      onUpgradeLatest={onUpgradeLatest}
    />
  );

  expect(screen.getByText(entry.display_name)).toBeInTheDocument();

  return { onInstall, onUpgradeLatest };
}

describe('OfficialPluginInstallPanel', () => {
  test('uses the official catalog icon url when provided', () => {
    renderPanel({
      ...baseEntry,
      icon: 'https://cdn.example.com/openai-compatible.svg'
    });

    expect(screen.getByAltText('')).toHaveAttribute(
      'src',
      'https://cdn.example.com/openai-compatible.svg'
    );
  });

  test('falls back to the default icon when the catalog entry has no icon', () => {
    renderPanel({
      ...baseEntry,
      icon: null
    });

    expect(screen.getByAltText('')).toHaveAttribute('src', '/icon.svg');
  });

  test('AC-002 marks a stale official catalog while the backend refreshes it', () => {
    renderPanel(
      baseEntry,
      {},
      {
        sourceKind: 'official_registry',
        sourceLabel: '官方源',
        registryUrl: 'https://example.com/official-registry.json',
        sourceFreshness: 'stale'
      }
    );

    expect(
      screen.getByText('正在显示上次成功获取的官方目录，后台正在刷新。')
    ).toBeInTheDocument();
  });

  test('asks for explicit host version risk acknowledgement before installing a below-minimum plugin', async () => {
    const onInstall = vi.fn();
    const belowMinimumEntry: SettingsOfficialPluginCatalogEntry = {
      ...baseEntry,
      latest_version: '0.3.0',
      current_host_version: '0.2.0',
      minimum_host_version: '0.3.0',
      compatibility_status: 'below_minimum_host_version',
      compatibility_warning_reason: 'below_minimum_host_version'
    };

    renderPanel(belowMinimumEntry, { onInstall });

    fireEvent.click(screen.getByRole('button', { name: '仍要安装' }));

    expect(
      await screen.findByText(
        '当前 1flowbase 版本低于该插件声明的最低适配版本。'
      )
    ).toBeInTheDocument();
    expect(screen.getByRole('dialog')).toHaveStyle({ width: '640px' });
    expect(screen.getByText('当前宿主版本：0.2.0')).toBeInTheDocument();
    expect(screen.getByText('插件最低适配版本：0.3.0')).toBeInTheDocument();
    expect(screen.getByText('插件版本：0.3.0')).toBeInTheDocument();

    const confirmButtons = screen.getAllByRole('button', {
      name: '仍要安装'
    });
    fireEvent.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() => {
      expect(onInstall).toHaveBeenCalledWith(belowMinimumEntry, {
        reason: 'below_minimum_host_version',
        acknowledged_current_host_version: '0.2.0',
        acknowledged_minimum_host_version: '0.3.0'
      });
    });
  });
});
