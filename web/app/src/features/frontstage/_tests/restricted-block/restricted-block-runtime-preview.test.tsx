import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { JsBlockTrialPanel } from '../../components/JsBlockTrialPanel';
import { RestrictedBlockRuntimePreview } from '../../components/RestrictedBlockRuntimePreview';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { RestrictedBlockRuntimeHostSnapshot } from '../../lib/restricted-block-runtime-host';

function createSnapshot(
  overrides: Partial<RestrictedBlockRuntimeHostSnapshot> = {}
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status: 'idle',
    requestId: 'restricted-block:block-1:code-1',
    blockId: 'block-1',
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

function createBlock(): FrontstageBlockInstance {
  return {
    id: 'block-1',
    rendererVersion: 'v1',
    sourceId: 'block-1',
    codeRef: 'code-1',
    sourceCodeRef: 'code-1',
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'metric.panel'
    },
    props: { title: 'Revenue' },
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 1 },
    order: 1,
    runtime: {
      kind: 'iframe',
      entry: 'blocks/metric/index.js',
      hint: 'iframe'
    }
  };
}

function createCatalogEntry(): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: 'official:metric.panel',
    runtimeKind: 'iframe',
    installationId: 'installation-1',
    providerCode: 'official',
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    contributionCode: 'metric.panel',
    title: 'Metric Panel',
    entry: 'blocks/metric/index.js',
    permissions: {
      network: 'none',
      storage: 'none',
      secrets: 'none'
    },
    contextContract: {
      primitives: ['text', 'button'],
      inputSchema: { type: 'object' }
    },
    uiCapabilities: ['responsive'],
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw']
  };
}

describe('RestrictedBlockRuntimePreview', () => {
  test('renders a ready snapshot with BlockUiRenderer and relays renderer actions through the injected callback', () => {
    const onAction = vi.fn();

    render(
      <RestrictedBlockRuntimePreview
        snapshot={createSnapshot({
          status: 'ready',
          schema: {
            primitive: 'Stack',
            children: [
              { primitive: 'Title', props: { children: 'Runtime Result' } },
              {
                primitive: 'Button',
                key: 'save-button',
                props: {
                  children: 'Save record',
                  actionId: 'record.save',
                  actionPayload: { id: 'record-1' }
                },
                permissions: { actions: ['record.save'] }
              }
            ]
          },
          logs: [
            {
              requestId: 'restricted-block:block-1:code-1',
              level: 'info',
              message: 'rendered',
              data: { hidden: 'raw-log-value' }
            }
          ],
          effects: [
            {
              type: 'action',
              requestId: 'restricted-block:block-1:code-1',
              effectId: 'effect-1',
              actionId: 'record.save',
              payload: { hidden: 'raw-effect-value' }
            }
          ],
          rejections: [
            {
              code: 'invalid_message',
              path: 'worker.message',
              message: 'Ignored malformed worker message.',
              requestId: 'restricted-block:block-1:code-1'
            }
          ]
        })}
        onAction={onAction}
        diagnostic
      />
    );

    expect(screen.queryByText('运行结果')).not.toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: 'Runtime Result' })
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Save record' }));
    expect(onAction).toHaveBeenCalledWith({
      type: 'action',
      primitive: 'Button',
      key: 'save-button',
      actionId: 'record.save',
      payload: { id: 'record-1' }
    });

    expect(screen.getByText('日志')).toBeInTheDocument();
    expect(screen.getByText('1 条')).toBeInTheDocument();
    expect(screen.getByText('效果')).toBeInTheDocument();
    expect(screen.getByText('动作 effect: record.save')).toBeInTheDocument();
    expect(screen.getByText('拒绝项')).toBeInTheDocument();
    expect(screen.getByText('invalid_message')).toBeInTheDocument();
    expect(screen.queryByText(/raw-log-value/)).not.toBeInTheDocument();
    expect(screen.queryByText(/raw-effect-value/)).not.toBeInTheDocument();
    expect(screen.queryByText(/\{"hidden"/)).not.toBeInTheDocument();
  });

  test('renders failed and timed out snapshots as controlled error summaries', () => {
    const { rerender } = render(
      <RestrictedBlockRuntimePreview
        diagnostic
        snapshot={createSnapshot({
          status: 'failed',
          error: {
            kind: 'runtime_error',
            message: 'Worker crashed while rendering.',
            errors: [
              {
                code: 'runtime_error',
                path: 'runtime.render',
                message: 'Worker crashed while rendering.'
              }
            ]
          }
        })}
      />
    );

    expect(screen.getByText('运行失败')).toBeInTheDocument();
    expect(screen.getAllByText('runtime_error').length).toBeGreaterThan(0);
    expect(screen.getByText('runtime.render')).toBeInTheDocument();
    expect(screen.getByText('Worker crashed while rendering.')).toBeInTheDocument();
    expect(screen.queryByText(/errors/)).not.toBeInTheDocument();

    rerender(
      <RestrictedBlockRuntimePreview
        diagnostic
        snapshot={createSnapshot({
          status: 'timed_out',
          error: {
            kind: 'runtime_timeout',
            message: 'JS block runtime timed out.',
            errors: [
              {
                code: 'runtime_timeout',
                path: 'runtime.timeout',
                message: 'JS block runtime timed out.'
              }
            ]
          }
        })}
      />
    );

    expect(screen.getByText('运行超时')).toBeInTheDocument();
    expect(screen.getAllByText('runtime_timeout').length).toBeGreaterThan(0);
    expect(screen.getByText('runtime.timeout')).toBeInTheDocument();
  });

  test('renders idle and running as local loading shells while keeping disposed explicit', () => {
    const { rerender } = render(
      <RestrictedBlockRuntimePreview snapshot={createSnapshot({ status: 'idle' })} />
    );

    expect(screen.getByTestId('block-ui-loading-shell')).toHaveAttribute(
      'aria-busy',
      'true'
    );
    expect(screen.queryByText('尚未运行')).not.toBeInTheDocument();
    expect(screen.queryByText(/restricted-block:block-1:code-1/)).not.toBeInTheDocument();

    rerender(
      <RestrictedBlockRuntimePreview
        snapshot={createSnapshot({
          status: 'running',
          logs: [
            {
              requestId: 'restricted-block:block-1:code-1',
              level: 'info',
              message: 'booting'
            }
          ]
        })}
      />
    );
    expect(screen.getByTestId('block-ui-loading-shell')).toBeInTheDocument();
    expect(screen.queryByText('运行中')).not.toBeInTheDocument();
    expect(screen.queryByText('booting')).not.toBeInTheDocument();
    expect(screen.queryByText('日志')).not.toBeInTheDocument();

    rerender(
      <RestrictedBlockRuntimePreview
        snapshot={createSnapshot({ status: 'disposed' })}
      />
    );
    expect(screen.queryByTestId('block-ui-loading-shell')).not.toBeInTheDocument();
    expect(screen.getByText('已释放')).toBeInTheDocument();
  });

  test('keeps JsBlockTrialPanel run plan UI without a snapshot and shows runtime results when a snapshot is injected', () => {
    const onRuntimeAction = vi.fn();
    const { rerender } = render(
      <JsBlockTrialPanel
        block={createBlock()}
        catalogEntry={createCatalogEntry()}
        code="export default { render() {} }"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={{ timeoutMs: 1000 }}
      />
    );

    expect(screen.getByText('Run plan 已生成')).toBeInTheDocument();
    expect(screen.queryByText('运行结果')).not.toBeInTheDocument();

    rerender(
      <JsBlockTrialPanel
        block={createBlock()}
        catalogEntry={createCatalogEntry()}
        code="export default { render() {} }"
        contextSnapshot={{ pageId: 'page-1' }}
        limits={{ timeoutMs: 1000 }}
        runtimeSnapshot={createSnapshot({
          status: 'ready',
          schema: {
            primitive: 'Stack',
            children: [
              { primitive: 'Text', props: { children: 'Rendered preview' } },
              {
                primitive: 'Button',
                key: 'panel-action',
                props: {
                  children: 'Panel action',
                  actionId: 'record.save'
                }
              }
            ]
          }
        })}
        onRuntimeAction={onRuntimeAction}
      />
    );

    const runtimeResult = screen.getByTestId('restricted-block-runtime-preview');
    expect(within(runtimeResult).queryByText('运行结果')).not.toBeInTheDocument();
    expect(within(runtimeResult).getByText('Rendered preview')).toBeInTheDocument();
    fireEvent.click(
      within(runtimeResult).getByRole('button', { name: 'Panel action' })
    );
    expect(onRuntimeAction).toHaveBeenCalledWith({
      type: 'action',
      primitive: 'Button',
      key: 'panel-action',
      actionId: 'record.save'
    });
    expect(screen.getByText('Run plan 已生成')).toBeInTheDocument();
  });
});
