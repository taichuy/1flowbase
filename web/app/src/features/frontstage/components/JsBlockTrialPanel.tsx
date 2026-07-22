import {
  createJsBlockDiagnostics,
  hashJsBlockDraft,
  type JsBlockHostEffectHandlers
} from '@1flowbase/page-runtime';
import { CloseOutlined } from '@ant-design/icons';
import {
  Button,
  Descriptions,
  Empty,
  List,
  Modal,
  Space,
  Tabs,
  Tag,
  Typography
} from 'antd';
import { useEffect, useMemo, useRef, useState } from 'react';

import { i18nText } from '../../../shared/i18n/text';
import type { FrontstageJsBlockCapabilityHandlers } from '../lib/js-block-capability-handlers';
import {
  createFrontstageRestrictedBlockRuntimeSession,
  type FrontstageRestrictedBlockRuntimeSession
} from '../lib/frontstage-restricted-block-runtime-host';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import type { FrontstageBlockInstance } from '../lib/page-document';
import {
  createRestrictedBlockRunPlan,
  type RestrictedBlockLoaderLimits
} from '../lib/restricted-block-loader';
import type { RestrictedBlockRuntimeHostSnapshot } from '../lib/restricted-block-runtime-host';
import { BlockRuntimeDiagnostics } from './BlockRuntimeDiagnostics';
import { RestrictedBlockRuntimePreview } from './RestrictedBlockRuntimePreview';
import { WindowWorkspaceWindow } from '../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { useOptionalWindowWorkspace } from '../../../shared/ui/window-workspace/WindowWorkspaceProvider';

export interface JsBlockTrialPanelProps {
  block: FrontstageBlockInstance;
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  code: string;
  contextSnapshot: Record<string, unknown>;
  handlers?: JsBlockHostEffectHandlers;
  onPrepareDraftRun?: FrontstageJsBlockCapabilityHandlers['prepareDraftRun'];
  onRevokeDraftRun?: FrontstageJsBlockCapabilityHandlers['revokeDraftRun'];
  limits: RestrictedBlockLoaderLimits;
  onCodeChange?: (code: string) => void;
  onContextSnapshotChange?: (value: Record<string, unknown>) => void;
  onLimitsChange?: (value: RestrictedBlockLoaderLimits) => void;
  runtimeSessionFactory?: typeof createFrontstageRestrictedBlockRuntimeSession;
}

export function JsBlockTrialPanel({
  block,
  catalogEntry,
  code,
  contextSnapshot,
  handlers,
  onPrepareDraftRun,
  onRevokeDraftRun,
  limits,
  runtimeSessionFactory = createFrontstageRestrictedBlockRuntimeSession
}: JsBlockTrialPanelProps) {
  const windowWorkspace = useOptionalWindowWorkspace();
  const sessionRef = useRef<FrontstageRestrictedBlockRuntimeSession | null>(
    null
  );
  const unsubscribeRef = useRef<(() => void) | null>(null);
  const [snapshot, setSnapshot] =
    useState<RestrictedBlockRuntimeHostSnapshot | null>(null);
  const [activeTab, setActiveTab] = useState('preview');
  const stop = () => {
    unsubscribeRef.current?.();
    unsubscribeRef.current = null;
    const session = sessionRef.current;
    sessionRef.current = null;
    if (session) {
      onRevokeDraftRun?.(session.getSnapshot().requestId);
      setSnapshot(session.dispose());
    }
  };

  useEffect(() => stop, []);

  const run = async () => {
    stop();
    if (!catalogEntry) return;
    const plan = createRestrictedBlockRunPlan({
      block,
      catalogEntry,
      code,
      contextSnapshot,
      inputs: {},
      limits
    });
    if (!plan.ok) {
      setSnapshot(createRejectedSnapshot(block.id, plan.message));
      return;
    }
    const runId = `draft:${block.id}:${Date.now().toString(36)}`;
    try {
      await onPrepareDraftRun?.({
        blockId: block.id,
        runId,
        draftHash: hashJsBlockDraft(code),
        confirmWrite: confirmWriteRun
      });
    } catch (error) {
      setSnapshot(
        createRejectedSnapshot(
          block.id,
          error instanceof Error ? error.message : String(error)
        )
      );
      return;
    }
    const runPlan = {
      ...plan,
      request: { ...plan.request, requestId: runId }
    };
    const session = runtimeSessionFactory({
      runPlan,
      handlers
    });
    sessionRef.current = session;
    unsubscribeRef.current = session.subscribe((next) => {
      setSnapshot(next);
      if (next.status !== 'running' && next.status !== 'idle') {
        onRevokeDraftRun?.(runId);
      }
    });
    setSnapshot(session.run());
  };

  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
        event.preventDefault();
        void run();
      }
    };
    window.addEventListener('keydown', keydown);
    return () => window.removeEventListener('keydown', keydown);
  });

  const diagnostics = useMemo(
    () =>
      snapshot?.error
        ? createJsBlockDiagnostics(
            {
              pageId: String(contextSnapshot.pageId ?? 'draft'),
              tabId: String(contextSnapshot.tabId ?? 'draft'),
              blockId: block.id
            },
            snapshot.error.errors
          )
        : [],
    [block.id, contextSnapshot.pageId, contextSnapshot.tabId, snapshot?.error]
  );

  const runPanels = [
    {
      key: 'preview',
      label: i18nText('frontstage', 'auto.preview'),
      content: snapshot ? (
        <RestrictedBlockRuntimePreview snapshot={snapshot} />
      ) : (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
      )
    },
    {
      key: 'console',
      label: i18nText('frontstage', 'auto.console'),
      content: <RunLogs snapshot={snapshot} />
    },
    {
      key: 'variables',
      label: i18nText('frontstage', 'auto.variables'),
      content: (
        <RunVariables snapshot={snapshot} contextSnapshot={contextSnapshot} />
      )
    },
    {
      key: 'interfaces',
      label: i18nText('frontstage', 'auto.interface_calls'),
      content: <RunInterfaceCalls snapshot={snapshot} />
    },
    {
      key: 'problems',
      label: i18nText('frontstage', 'auto.problems'),
      content:
        diagnostics.length > 0 ? (
          <BlockRuntimeDiagnostics diagnostics={diagnostics} />
        ) : (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
        )
    }
  ] as const;
  const mainWindowId = `frontstage-jsx-studio:${block.codeRef}`;
  const openRunPanel = (key: string, index: number) => {
    windowWorkspace?.open({
      id: `${mainWindowId}:run:${key}`,
      owner: mainWindowId,
      parent_id: mainWindowId,
      rect: {
        left: 180 + index * 28,
        top: 110 + index * 28,
        width: 640,
        height: 440
      },
      dirty: false
    });
  };

  return (
    <Space direction="vertical" size={12} style={{ width: '100%' }}>
      <Space>
        <Button type="primary" onClick={() => void run()}>
          {i18nText('frontstage', 'auto.run')}
        </Button>
        <Button disabled={!sessionRef.current} onClick={stop}>
          {i18nText('frontstage', 'auto.stop')}
        </Button>
        {snapshot ? <Tag>{snapshot.requestId}</Tag> : null}
      </Space>
      {windowWorkspace ? (
        <>
          <Space wrap>
            {runPanels.map((panel, index) => (
              <Button
                key={panel.key}
                onClick={() => openRunPanel(panel.key, index)}
              >
                {panel.label}
              </Button>
            ))}
          </Space>
          {runPanels.map((panel) => {
            const id = `${mainWindowId}:run:${panel.key}`;
            const entry = windowWorkspace.state.windows.find(
              (candidate) => candidate.id === id
            );
            if (!entry) return null;
            const topZ = Math.max(
              ...windowWorkspace.state.windows.map(
                (candidate) => candidate.z_index
              )
            );
            return (
              <WindowWorkspaceWindow
                key={id}
                active={entry.z_index === topZ}
                className="frontstage-jsx-studio__run-window"
                bodyClassName="frontstage-jsx-studio__run-window-body"
                dragHandleSelector="[data-window-drag-handle='true']"
                initialRect={() => entry.rect}
                rect={entry.rect}
                resizeLabel={(edge) => `${panel.label} ${edge}`}
                testId={id}
                title={panel.label}
                zIndex={1050 + entry.z_index}
                onActivate={() => windowWorkspace.activate(id)}
                onRectChange={(rect) => windowWorkspace.setRect(id, rect)}
              >
                <header
                  className="frontstage-jsx-studio__run-window-header"
                  data-window-drag-handle="true"
                >
                  <Typography.Text strong>{panel.label}</Typography.Text>
                  <Button
                    aria-label={i18nText('frontstage', 'auto.close')}
                    icon={<CloseOutlined />}
                    size="small"
                    type="text"
                    onClick={() => windowWorkspace.close(id)}
                  />
                </header>
                <div className="frontstage-jsx-studio__run-window-content">
                  {panel.content}
                </div>
              </WindowWorkspaceWindow>
            );
          })}
        </>
      ) : (
        <Tabs
          activeKey={activeTab}
          onChange={setActiveTab}
          items={runPanels.map((panel) => ({
            key: panel.key,
            label: panel.label,
            children: panel.content
          }))}
        />
      )}
    </Space>
  );
}

function RunLogs({
  snapshot
}: {
  snapshot: RestrictedBlockRuntimeHostSnapshot | null;
}) {
  return snapshot?.logs.length ? (
    <List
      dataSource={snapshot.logs}
      renderItem={(log) => (
        <List.Item>
          <Space>
            <Tag>{log.level}</Tag>
            <Typography.Text>{log.message}</Typography.Text>
          </Space>
        </List.Item>
      )}
    />
  ) : (
    <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
  );
}

function RunVariables({
  snapshot,
  contextSnapshot
}: {
  snapshot: RestrictedBlockRuntimeHostSnapshot | null;
  contextSnapshot: Record<string, unknown>;
}) {
  const values = {
    ...Object.fromEntries(
      Object.entries(contextSnapshot).map(([key, value]) => [
        `context.${key}`,
        value
      ])
    ),
    ...Object.fromEntries(
      Object.entries(snapshot?.outputs ?? {}).map(([key, value]) => [
        `outputs.${key}`,
        value
      ])
    )
  };
  return (
    <Descriptions
      column={1}
      size="small"
      items={Object.entries(values).map(([key, value]) => ({
        key,
        label: key,
        children: formatValue(value)
      }))}
    />
  );
}

function RunInterfaceCalls({
  snapshot
}: {
  snapshot: RestrictedBlockRuntimeHostSnapshot | null;
}) {
  return snapshot?.interfaceCalls?.length ? (
    <List
      dataSource={snapshot.interfaceCalls}
      renderItem={(call) => (
        <List.Item>
          <Space direction="vertical">
            <Typography.Text code>
              {call.method} {call.path}
            </Typography.Text>
            <Typography.Text type="secondary">
              {call.status} · {call.durationMs}ms
            </Typography.Text>
          </Space>
        </List.Item>
      )}
    />
  ) : (
    <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
  );
}

function formatValue(value: unknown): string {
  if (value === null) return 'null';
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean')
    return String(value);
  if (Array.isArray(value)) return `${value.length} items`;
  return value && typeof value === 'object'
    ? `${Object.keys(value).length} fields`
    : 'undefined';
}

function confirmWriteRun(): Promise<boolean> {
  return new Promise((resolve) => {
    Modal.confirm({
      title: i18nText('frontstage', 'auto.confirm_write_run'),
      content: i18nText('frontstage', 'auto.confirm_write_run_description'),
      onOk: () => resolve(true),
      onCancel: () => resolve(false)
    });
  });
}

function createRejectedSnapshot(
  blockId: string,
  message: string
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status: 'failed',
    requestId: `draft:${blockId}:rejected`,
    blockId,
    schemaValidationOptions: {},
    error: {
      kind: 'runtime_error',
      message,
      errors: [{ code: 'runtime_error', path: 'runPlan', message }]
    },
    logs: [],
    effects: [],
    rejections: [],
    interfaceCalls: []
  };
}
