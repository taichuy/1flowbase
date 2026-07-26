import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { AddBlockCatalogPickerDrawer } from '../../components/AddBlockCatalogPickerDrawer';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';

function createCatalogEntry(
  overrides: Partial<NormalizedFrontstageBlockCatalogEntry> = {}
): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: '1flowbase:frontstage.js-ui-block',
    runtimeKind: 'native_react',
    installationId: 'builtin-installation',
    providerCode: '1flowbase',
    pluginId: 'builtin-frontstage',
    pluginVersion: '1.0.0',
    contributionCode: 'frontstage.js-ui-block',
    title: '空白 JS Block',
    entry: 'index.js',
    permissions: {
      network: 'none',
      storage: 'none',
      secrets: 'none'
    },
    contextContract: {
      primitives: [],
      inputSchema: {}
    },
    uiCapabilities: [],
    codeCapabilities: {
      template: {
        source: 'export default { render: () => null };',
        version: '2.4.0',
        language: 'tsx'
      },
      allowedImports: [],
      monacoExtraLibs: []
    },
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw'],
    ...overrides
  };
}

describe('AddBlockCatalogPickerDrawer', () => {
  test('shows a clear empty state when no catalog entries are available', () => {
    render(
      <AddBlockCatalogPickerDrawer
        open
        items={[]}
        onSelect={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(
      screen.getByRole('dialog', { name: '新增区块' })
    ).toBeInTheDocument();
    expect(
      screen.getByText('当前没有可用区块目录项，暂时无法新增区块。')
    ).toBeInTheDocument();
  });

  test('selects the catalog entry without a second template choice', () => {
    const onSelect = vi.fn();
    const entry = createCatalogEntry();
    render(
      <AddBlockCatalogPickerDrawer
        open
        items={[entry]}
        onSelect={onSelect}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByText('空白 JS Block')).toBeInTheDocument();
    expect(screen.getByText('iframe')).toBeInTheDocument();
    expect(screen.getByText('1flowbase')).toBeInTheDocument();
    expect(screen.getByText('frontstage.js-ui-block')).toBeInTheDocument();
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '选择' }));

    expect(onSelect).toHaveBeenCalledWith(entry);
  });

  test('disables entries without a catalog code template', () => {
    const entry = createCatalogEntry({ codeCapabilities: undefined });
    render(
      <AddBlockCatalogPickerDrawer
        open
        items={[entry]}
        onSelect={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: '选择' })).toBeDisabled();
  });

  test('disables catalog selection while saving or loading', () => {
    const entry = createCatalogEntry();
    const { rerender } = render(
      <AddBlockCatalogPickerDrawer
        open
        items={[entry]}
        saving
        onSelect={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: '选择' })).toBeDisabled();

    rerender(
      <AddBlockCatalogPickerDrawer
        open
        items={[entry]}
        loading
        onSelect={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: '选择' })).toBeDisabled();
  });
});
