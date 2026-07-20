import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { appI18n } from '../../../shared/i18n/app-i18n';
import { JsBlockTrialPanel } from '../components/JsBlockTrialPanel';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import type { FrontstageRestrictedBlockRuntimeSession } from '../lib/frontstage-restricted-block-runtime-host';
import type { FrontstageBlockInstance } from '../lib/page-document';
import type { RestrictedBlockRuntimeHostSnapshot } from '../lib/restricted-block-runtime-host';

const block = {
  id: 'block-1',
  rendererVersion: 'v1',
  sourceId: 'block-1',
  codeRef: 'code-1',
  sourceCodeRef: 'code-1',
  catalog: { providerCode: 'official', installationId: 'installation-1' },
  contribution: {
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    code: 'tsx'
  },
  props: {},
  interfaces: [],
  ports: { inputs: [], outputs: [] },
  presentation: { heightMode: 'auto', height: null },
  layout: { order: 0 },
  order: 0,
  runtime: { kind: 'iframe', entry: 'index.js', hint: 'iframe' }
} satisfies FrontstageBlockInstance;

const catalog = {
  id: 'official:tsx',
  runtimeKind: 'iframe',
  installationId: 'installation-1',
  providerCode: 'official',
  pluginId: 'official.blocks',
  pluginVersion: '1.0.0',
  contributionCode: 'tsx',
  title: 'TSX',
  entry: 'index.js',
  permissions: { network: 'none', storage: 'none', secrets: 'none' },
  contextContract: { primitives: [], inputSchema: {} },
  uiCapabilities: [],
  codeCapabilities: {
    template: null,
    allowedImports: [],
    monacoExtraLibs: [],
    workerModuleSources: []
  },
  raw: {}
} as unknown as NormalizedFrontstageBlockCatalogEntry;

function snapshot(
  status: RestrictedBlockRuntimeHostSnapshot['status']
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status,
    requestId: 'draft:block-1:run',
    blockId: 'block-1',
    schemaValidationOptions: {},
    ...(status === 'ready'
      ? {
          view: { primitive: 'Text', props: { children: 'Ready' } },
          outputs: { total: 2 }
        }
      : {}),
    logs: [],
    effects: [],
    rejections: [],
    interfaceCalls: []
  };
}

function createSession(): FrontstageRestrictedBlockRuntimeSession {
  let current = snapshot('idle');
  const listeners = new Set<
    (value: RestrictedBlockRuntimeHostSnapshot) => void
  >();
  return {
    run: vi.fn(() => {
      current = snapshot('running');
      return current;
    }),
    dispose: vi.fn(() => {
      current = snapshot('disposed');
      return current;
    }),
    getSnapshot: vi.fn(() => current),
    getHostState: vi.fn(() => ({
      workerStatus: 'idle' as const,
      requests: {},
      rejections: []
    })),
    subscribe: vi.fn((listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    })
  };
}

describe('JsBlockTrialPanel Draft Run Console', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await appI18n.changeLanguage('zh_Hans');
  });

  test('AC-008 runs the unsaved draft without exposing raw context or limits editors', async () => {
    const runtimeSessionFactory = vi.fn(() => createSession());
    render(
      <JsBlockTrialPanel
        block={block}
        catalogEntry={catalog}
        code={
          'async function main(){return {view:{primitive:"Text"},outputs:{}}}\nexport default {main};'
        }
        contextSnapshot={{ pageId: 'page-1' }}
        limits={{ timeoutMs: 1_000 }}
        runtimeSessionFactory={runtimeSessionFactory}
      />
    );
    expect(screen.queryByText('Runtime limits')).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '运行' }));
    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(1));
    expect(screen.getByText('预览')).toBeInTheDocument();
    expect(screen.getByText('控制台')).toBeInTheDocument();
    expect(screen.getByText('接口调用')).toBeInTheDocument();
    expect(screen.getByText('问题')).toBeInTheDocument();
  });
});
