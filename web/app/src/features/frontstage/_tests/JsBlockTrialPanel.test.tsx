import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { BlockRendererActionEvent } from '@1flowbase/block-renderer';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { appI18n } from '../../../shared/i18n/app-i18n';
import { JsBlockTrialPanel } from '../components/JsBlockTrialPanel';
import { WindowWorkspaceProvider } from '../../../shared/ui/window-workspace/WindowWorkspaceProvider';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import type {
  FrontstageRestrictedBlockRuntimeHostOptions,
  FrontstageRestrictedBlockRuntimeSession
} from '../lib/frontstage-restricted-block-runtime-host';
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

function createReadyActionSession(): FrontstageRestrictedBlockRuntimeSession {
  const ready = {
    ...snapshot('ready'),
    schemaValidationOptions: { allowedActions: ['sign_up'] },
    view: {
      primitive: 'Button',
      props: { children: 'Register', actionId: 'sign_up' }
    }
  } satisfies RestrictedBlockRuntimeHostSnapshot;
  const disposed = {
    ...ready,
    status: 'disposed'
  } satisfies RestrictedBlockRuntimeHostSnapshot;
  return {
    run: vi.fn(() => ready),
    dispose: vi.fn(() => disposed),
    getSnapshot: vi.fn(() => ready),
    getHostState: vi.fn(() => ({
      workerStatus: 'idle' as const,
      requests: {},
      rejections: []
    })),
    subscribe: vi.fn(() => () => undefined)
  };
}

describe('JsBlockTrialPanel Draft Run Console', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await appI18n.changeLanguage('zh_Hans');
  });

  afterEach(() => {
    vi.useRealTimers();
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
        presentation="debugger"
        runtimeSessionFactory={runtimeSessionFactory}
      />
    );
    expect(screen.queryByText('Runtime limits')).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /运\s*行/ }));
    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(1));
    expect(screen.getByText('预览')).toBeInTheDocument();
    expect(screen.getByText('控制台')).toBeInTheDocument();
    expect(screen.getByText('接口调用')).toBeInTheDocument();
    expect(screen.getByText('问题')).toBeInTheDocument();
  });

  test('AC-010 opens Draft Run surfaces as independent child windows in Studio', () => {
    render(
      <WindowWorkspaceProvider>
        <JsBlockTrialPanel
          block={block}
          catalogEntry={catalog}
          code="async function main(){return {view:{primitive:'Text'},outputs:{}}} export default {main};"
          contextSnapshot={{ pageId: 'page-1' }}
          limits={{ timeoutMs: 1_000 }}
          presentation="debugger"
          runtimeSessionFactory={() => createSession()}
        />
      </WindowWorkspaceProvider>
    );

    fireEvent.click(screen.getByRole('button', { name: /预\s*览/ }));
    fireEvent.click(screen.getByRole('button', { name: '控制台' }));
    expect(screen.getByRole('dialog', { name: '预览' })).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: '控制台' })).toBeInTheDocument();
  });

  test('AC-032 reruns the current draft with host inputs derived from an action event', async () => {
    const createRunInputs = vi.fn((event?: BlockRendererActionEvent) => ({
      authenticator_id: 'auth-password-local',
      public_variables: { self_registration_enabled: true },
      ...(event
        ? {
            auth_event: {
              action_id: event.actionId,
              values: event.formValues ?? {}
            }
          }
        : {})
    }));
    const runInputs: Array<Record<string, unknown> | undefined> = [];
    const runtimeSessionFactory = vi.fn(
      (options: FrontstageRestrictedBlockRuntimeHostOptions) => {
        runInputs.push(options.runPlan.request.inputs);
        return createReadyActionSession();
      }
    );
    render(
      <JsBlockTrialPanel
        block={block}
        catalogEntry={catalog}
        code="async function main(){return {view:{primitive:'Button'},outputs:{}}} export default {main};"
        contextSnapshot={{}}
        createRunInputs={createRunInputs}
        limits={{ timeoutMs: 1_000 }}
        presentation="debugger"
        runtimeSessionFactory={runtimeSessionFactory}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /运\s*行/ }));
    await screen.findByRole('button', { name: 'Register' });
    fireEvent.click(screen.getByRole('button', { name: 'Register' }));

    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(2));
    expect(createRunInputs).toHaveBeenNthCalledWith(1, undefined);
    expect(createRunInputs).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ actionId: 'sign_up' })
    );
    expect(runInputs[1]).toEqual({
      authenticator_id: 'auth-password-local',
      public_variables: { self_registration_enabled: true },
      auth_event: { action_id: 'sign_up', values: {} }
    });
  });

  test('AC-035/036 directly renders and debounces the latest draft without debugger controls', async () => {
    vi.useFakeTimers();
    const sessions = [
      createReadyActionSession(),
      createReadyActionSession(),
      createReadyActionSession()
    ];
    let nextSession = 0;
    const runSources: string[] = [];
    const runtimeSessionFactory = vi.fn(
      (options: FrontstageRestrictedBlockRuntimeHostOptions) => {
        const program = options.runPlan.request.program;
        runSources.push(
          program.kind === 'source' ? program.source : program.fallback.source
        );
        const session = sessions[nextSession++];
        if (!session) throw new Error('missing test session');
        return session;
      }
    );
    const createRunInputs = vi.fn((event?: BlockRendererActionEvent) => ({
      ...(event ? { auth_event: { action_id: event.actionId } } : {})
    }));
    const { rerender } = render(
      <JsBlockTrialPanel
        block={block}
        catalogEntry={catalog}
        code="first draft"
        contextSnapshot={{}}
        createRunInputs={createRunInputs}
        limits={{ timeoutMs: 1_000 }}
        presentation="direct-preview"
        runtimeSessionFactory={runtimeSessionFactory}
      />
    );

    for (const name of [
      /运\s*行/,
      '停止',
      '预览',
      '控制台',
      '变量',
      '接口调用',
      '问题'
    ]) {
      expect(screen.queryByRole('button', { name })).not.toBeInTheDocument();
    }

    await act(async () => vi.runAllTimersAsync());
    expect(runtimeSessionFactory).toHaveBeenCalledTimes(1);
    expect(runSources).toEqual(['first draft']);
    expect(screen.getByRole('button', { name: 'Register' })).toBeInTheDocument();

    rerender(
      <JsBlockTrialPanel
        block={block}
        catalogEntry={catalog}
        code="intermediate draft"
        contextSnapshot={{}}
        createRunInputs={createRunInputs}
        limits={{ timeoutMs: 1_000 }}
        presentation="direct-preview"
        runtimeSessionFactory={runtimeSessionFactory}
      />
    );
    rerender(
      <JsBlockTrialPanel
        block={block}
        catalogEntry={catalog}
        code="latest draft"
        contextSnapshot={{}}
        createRunInputs={createRunInputs}
        limits={{ timeoutMs: 1_000 }}
        presentation="direct-preview"
        runtimeSessionFactory={runtimeSessionFactory}
      />
    );
    await act(async () => vi.runAllTimersAsync());
    expect(runtimeSessionFactory).toHaveBeenCalledTimes(2);
    expect(runSources).toEqual(['first draft', 'latest draft']);
    expect(sessions[0]?.dispose).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(/^draft:/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Register' }));
    await act(async () => Promise.resolve());
    expect(runtimeSessionFactory).toHaveBeenCalledTimes(3);
    expect(createRunInputs).toHaveBeenLastCalledWith(
      expect.objectContaining({ actionId: 'sign_up' })
    );
  });
});
