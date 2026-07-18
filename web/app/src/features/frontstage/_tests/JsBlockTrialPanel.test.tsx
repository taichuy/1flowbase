import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { appI18n } from '../../../shared/i18n/app-i18n';
import { JsBlockTrialPanel } from '../components/JsBlockTrialPanel';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import type { FrontstageRestrictedBlockRuntimeSession } from '../lib/frontstage-restricted-block-runtime-host';
import type { FrontstageBlockInstance } from '../lib/page-document';
import type { RestrictedBlockLoaderLimits } from '../lib/restricted-block-loader';
import type { RestrictedBlockRuntimeHostSnapshot } from '../lib/restricted-block-runtime-host';

function createBlock(
  overrides: Partial<FrontstageBlockInstance> = {}
): FrontstageBlockInstance {
  return {
    id: 'hero-block',
    sourceId: 'hero-block',
    codeRef: 'hero-code',
    sourceCodeRef: 'hero-code',
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'hero.banner'
    },
    props: { title: 'Hello' },
    layout: { order: 1 },
    order: 1,
    runtime: {
      kind: 'iframe',
      entry: 'blocks/hero/index.js',
      hint: 'iframe'
    },
    ...overrides,
    rendererVersion: overrides.rendererVersion ?? 'v1'
  };
}

function createCatalogEntry(
  overrides: Partial<NormalizedFrontstageBlockCatalogEntry> = {}
): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: 'official:hero.banner',
    runtimeKind: 'iframe',
    installationId: 'installation-1',
    providerCode: 'official',
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    contributionCode: 'hero.banner',
    title: 'Hero Banner',
    entry: 'blocks/hero/index.js',
    permissions: {
      network: 'none',
      storage: 'none',
      secrets: 'none'
    },
    contextContract: {
      primitives: ['text', 'button', 'data_record'],
      inputSchema: { type: 'object' }
    },
    uiCapabilities: ['responsive', 'data_binding'],
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw'],
    ...overrides
  };
}

function createLimits(
  overrides: Partial<RestrictedBlockLoaderLimits> = {}
): RestrictedBlockLoaderLimits {
  return {
    timeoutMs: 1000,
    maxRenderDepth: 8,
    maxRenderNodes: 250,
    allowedActions: ['record.save'],
    allowedEvents: ['record.saved'],
    allowedQueries: ['records'],
    allowedDataOperations: ['query'],
    maxEventChainDepth: 4,
    ...overrides
  };
}

function createSnapshot(
  overrides: Partial<RestrictedBlockRuntimeHostSnapshot> = {}
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status: 'idle',
    requestId: 'restricted-block:hero-block:hero-code',
    blockId: 'hero-block',
    schemaValidationOptions: {
      maxDepth: 8,
      maxNodes: 250,
      allowedActions: ['record.save'],
      allowedEvents: ['record.saved'],
      allowedDataPermissions: ['query']
    },
    logs: [],
    effects: [],
    rejections: [],
    ...overrides
  };
}

function createFakeRuntimeSession() {
  type SnapshotListener = Parameters<
    FrontstageRestrictedBlockRuntimeSession['subscribe']
  >[0];
  type RuntimeSessionState = ReturnType<
    FrontstageRestrictedBlockRuntimeSession['getHostState']
  >;

  let snapshot = createSnapshot();
  const listeners = new Set<SnapshotListener>();
  const session: FrontstageRestrictedBlockRuntimeSession = {
    run: vi.fn(() => {
      snapshot = createSnapshot({ status: 'running' });
      return snapshot;
    }),
    dispose: vi.fn(() => {
      snapshot = createSnapshot({ status: 'disposed' });
      return snapshot;
    }),
    getSnapshot: vi.fn(() => snapshot),
    getHostState: vi.fn(
      () =>
        ({
          workerStatus: 'idle',
          requests: {},
          rejections: []
        }) satisfies RuntimeSessionState
    ),
    subscribe: vi.fn((listener: SnapshotListener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    })
  };

  return {
    session,
    emit(nextSnapshot: RestrictedBlockRuntimeHostSnapshot) {
      snapshot = nextSnapshot;
      for (const listener of [...listeners]) {
        listener(snapshot);
      }
    }
  };
}

describe('JsBlockTrialPanel', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
  });

  test('shows clear empty states when the selected block or catalog entry is missing', () => {
    const { rerender } = render(
      <JsBlockTrialPanel
        block={null}
        catalogEntry={createCatalogEntry()}
        code="export default {}"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={createLimits()}
      />
    );

    expect(screen.getByText('请选择一个区块')).toBeInTheDocument();

    rerender(
      <JsBlockTrialPanel
        block={createBlock()}
        catalogEntry={null}
        code="export default {}"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={createLimits()}
      />
    );

    expect(screen.getByText('缺少区块目录条目')).toBeInTheDocument();
  });

  test('renders a valid run plan summary without executing JavaScript', () => {
    render(
      <JsBlockTrialPanel
        block={createBlock()}
        catalogEntry={createCatalogEntry()}
        code="export default { render() {} }"
        contextSnapshot={{ pageId: 'page-1', locale: 'zh_Hans' }}
        limits={createLimits()}
      />
    );

    expect(screen.getByText('Run plan 已生成')).toBeInTheDocument();
    expect(
      screen.getByText('restricted-block:hero-block:hero-code')
    ).toBeInTheDocument();
    expect(screen.getByText('hero-block')).toBeInTheDocument();
    expect(screen.getByText('1000ms')).toBeInTheDocument();
    expect(screen.getByText('pageId, locale')).toBeInTheDocument();

    const schemaOptions = screen.getByTestId('js-block-trial-schema-options');
    expect(within(schemaOptions).getByText('8')).toBeInTheDocument();
    expect(within(schemaOptions).getByText('250')).toBeInTheDocument();
    expect(within(schemaOptions).getByText('query')).toBeInTheDocument();
    expect(within(schemaOptions).getByText('record.save')).toBeInTheDocument();

    const mediatorPolicy = screen.getByTestId('js-block-trial-mediator-policy');
    expect(within(mediatorPolicy).getByText('records')).toBeInTheDocument();
    expect(within(mediatorPolicy).getByText('4')).toBeInTheDocument();
  });

  test('runs an injected runtime session and renders subscribed runtime snapshots', () => {
    const runtimeSession = createFakeRuntimeSession();
    const runtimeSessionFactory = vi.fn(() => runtimeSession.session);

    render(
      <JsBlockTrialPanel
        block={createBlock()}
        catalogEntry={createCatalogEntry()}
        code="export default { render() {} }"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={createLimits()}
        runtimeSessionFactory={runtimeSessionFactory}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '运行' }));

    expect(runtimeSessionFactory).toHaveBeenCalledTimes(1);
    expect(runtimeSessionFactory).toHaveBeenCalledWith({
      runPlan: expect.objectContaining({
        ok: true,
        request: expect.objectContaining({
          requestId: 'restricted-block:hero-block:hero-code',
          blockId: 'hero-block'
        })
      })
    });
    expect(runtimeSession.session.subscribe).toHaveBeenCalledTimes(1);
    expect(runtimeSession.session.run).toHaveBeenCalledTimes(1);
    expect(screen.getByText('运行中')).toBeInTheDocument();

    act(() => {
      runtimeSession.emit(
        createSnapshot({
          status: 'ready',
          schema: {
            primitive: 'Text',
            props: { children: 'Runtime Ready' }
          }
        })
      );
    });

    expect(screen.getByText('运行结果')).toBeInTheDocument();
    expect(screen.getByText('Runtime Ready')).toBeInTheDocument();

    act(() => {
      runtimeSession.emit(
        createSnapshot({
          status: 'failed',
          error: {
            kind: 'runtime_error',
            message: 'Worker failed.',
            errors: [
              {
                code: 'runtime_error',
                path: 'runtime',
                message: 'Worker failed.'
              }
            ]
          }
        })
      );
    });

    expect(screen.getByText('运行失败')).toBeInTheDocument();
    expect(screen.getByText('Worker failed.')).toBeInTheDocument();
  });

  test('injects the configured data effect handler into runtime sessions', () => {
    const runtimeSession = createFakeRuntimeSession();
    const runtimeSessionFactory = vi.fn(() => runtimeSession.session);
    const dataEffectHandler = vi.fn(async () => ({ ok: true }));

    render(
      <JsBlockTrialPanel
        block={createBlock()}
        catalogEntry={createCatalogEntry()}
        code="export default { render() {} }"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={createLimits()}
        runtimeSessionFactory={runtimeSessionFactory}
        handlers={{ data: dataEffectHandler }}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '运行' }));

    expect(runtimeSessionFactory).toHaveBeenCalledWith({
      runPlan: expect.objectContaining({
        ok: true,
        request: expect.objectContaining({
          requestId: 'restricted-block:hero-block:hero-code'
        })
      }),
      handlers: { data: dataEffectHandler }
    });
  });

  test('disposes the active runtime session before rerun, on stop, and on unmount', () => {
    const firstRuntimeSession = createFakeRuntimeSession();
    const secondRuntimeSession = createFakeRuntimeSession();
    const thirdRuntimeSession = createFakeRuntimeSession();
    const sessions = [
      firstRuntimeSession.session,
      secondRuntimeSession.session,
      thirdRuntimeSession.session
    ];
    const runtimeSessionFactory = vi.fn(() => sessions.shift()!);

    const { unmount } = render(
      <JsBlockTrialPanel
        block={createBlock()}
        catalogEntry={createCatalogEntry()}
        code="export default { render() {} }"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={createLimits()}
        runtimeSessionFactory={runtimeSessionFactory}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '运行' }));
    expect(firstRuntimeSession.session.run).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: '运行' }));
    expect(firstRuntimeSession.session.dispose).toHaveBeenCalledTimes(1);
    expect(secondRuntimeSession.session.run).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: '停止' }));
    expect(secondRuntimeSession.session.dispose).toHaveBeenCalledTimes(1);
    expect(screen.getByText('已释放')).toBeInTheDocument();

    act(() => {
      secondRuntimeSession.emit(
        createSnapshot({
          status: 'ready',
          schema: {
            primitive: 'Text',
            props: { children: 'Late runtime result' }
          }
        })
      );
    });
    expect(screen.queryByText('Late runtime result')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '运行' }));
    expect(thirdRuntimeSession.session.run).toHaveBeenCalledTimes(1);

    unmount();
    expect(thirdRuntimeSession.session.dispose).toHaveBeenCalledTimes(1);
  });

  test('does not expose runtime run controls or create sessions when the run plan is rejected', () => {
    const runtimeSessionFactory = vi.fn(() => createFakeRuntimeSession().session);

    render(
      <JsBlockTrialPanel
        block={createBlock({ codeRef: '' })}
        catalogEntry={createCatalogEntry()}
        code="export default {}"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={createLimits()}
        runtimeSessionFactory={runtimeSessionFactory}
      />
    );

    expect(screen.getByText('Run plan 被拒绝')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '运行' })
    ).not.toBeInTheDocument();
    expect(runtimeSessionFactory).not.toHaveBeenCalled();
  });

  test('renders structured rejection details from the run plan builder', () => {
    render(
      <JsBlockTrialPanel
        block={createBlock({ codeRef: '' })}
        catalogEntry={createCatalogEntry()}
        code="export default {}"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={createLimits()}
      />
    );

    expect(screen.getByText('Run plan 被拒绝')).toBeInTheDocument();
    expect(screen.getByText('missing_code_ref')).toBeInTheDocument();
    expect(screen.getByText('block.codeRef')).toBeInTheDocument();
    expect(
      screen.getByText('Restricted block codeRef is required.')
    ).toBeInTheDocument();
    expect(screen.getByText('hero-block')).toBeInTheDocument();
    expect(screen.getByText('official:hero.banner')).toBeInTheDocument();
  });

  test('offers controlled editors for code, context snapshot, and limits', () => {
    const onCodeChange = vi.fn();
    const onContextSnapshotChange = vi.fn();
    const onLimitsChange = vi.fn();

    function Harness() {
      const [code, setCode] = useState('export default {}');
      const [contextSnapshot, setContextSnapshot] = useState<
        Record<string, unknown>
      >({
        pageId: 'page-1'
      });
      const [limits, setLimits] = useState<RestrictedBlockLoaderLimits>(
        createLimits()
      );

      return (
        <JsBlockTrialPanel
          block={createBlock()}
          catalogEntry={createCatalogEntry()}
          code={code}
          contextSnapshot={contextSnapshot}
          limits={limits}
          onCodeChange={(nextCode) => {
            onCodeChange(nextCode);
            setCode(nextCode);
          }}
          onContextSnapshotChange={(nextContextSnapshot) => {
            onContextSnapshotChange(nextContextSnapshot);
            setContextSnapshot(nextContextSnapshot);
          }}
          onLimitsChange={(nextLimits) => {
            onLimitsChange(nextLimits);
            setLimits(nextLimits);
          }}
        />
      );
    }

    render(<Harness />);

    fireEvent.change(screen.getByLabelText('JS 代码'), {
      target: { value: 'export default { render() { return null } }' }
    });
    expect(onCodeChange).toHaveBeenCalledWith(
      'export default { render() { return null } }'
    );

    fireEvent.change(screen.getByLabelText('上下文快照'), {
      target: { value: '{ "pageId": "page-2", "recordId": "record-1" }' }
    });
    fireEvent.click(screen.getByRole('button', { name: '更新 context' }));
    expect(onContextSnapshotChange).toHaveBeenCalledWith({
      pageId: 'page-2',
      recordId: 'record-1'
    });
    expect(screen.getByText('pageId, recordId')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('运行时限制'), {
      target: {
        value:
          '{ "timeoutMs": 2000, "maxRenderDepth": 4, "maxRenderNodes": 120, "allowedActions": ["record.archive"], "allowedEvents": [], "allowedQueries": ["cases"], "allowedDataOperations": ["query"], "maxEventChainDepth": 2 }'
      }
    });
    fireEvent.click(screen.getByRole('button', { name: '更新 limits' }));
    expect(onLimitsChange).toHaveBeenCalledWith({
      timeoutMs: 2000,
      maxRenderDepth: 4,
      maxRenderNodes: 120,
      allowedActions: ['record.archive'],
      allowedEvents: [],
      allowedQueries: ['cases'],
      allowedDataOperations: ['query'],
      maxEventChainDepth: 2
    });
    expect(screen.getByText('2000ms')).toBeInTheDocument();
  });

  test('rejects invalid runtime limits drafts before emitting changes', () => {
    const onLimitsChange = vi.fn();

    render(
      <JsBlockTrialPanel
        block={createBlock()}
        catalogEntry={createCatalogEntry()}
        code="export default {}"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={createLimits()}
        onLimitsChange={onLimitsChange}
      />
    );

    fireEvent.change(screen.getByLabelText('运行时限制'), {
      target: { value: '{ "maxRenderDepth": 4 }' }
    });
    fireEvent.click(screen.getByRole('button', { name: '更新 limits' }));

    expect(
      screen.getByText('Runtime limits.timeoutMs 必须是正数。')
    ).toBeInTheDocument();
    expect(onLimitsChange).not.toHaveBeenCalled();
  });
});
