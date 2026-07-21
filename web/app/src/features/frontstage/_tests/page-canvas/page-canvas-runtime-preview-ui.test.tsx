import { render, screen, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import type { FrontstagePageContent } from '../../api/page-content';
import { PageCanvas } from '../../components/PageCanvas';
import {
  createFrontstagePageContentFixture,
  type FrontstagePageContentFixtureOverrides
} from '../frontstage-page-content-fixtures';
import type { FrontstagePageCanvasRuntimeSessionEntry } from '../../hooks/use-frontstage-page-canvas-runtime-sessions';
import type { FrontstagePageCanvasRuntimeRunPlanState } from '../../lib/page-canvas/runtime-run-plan';
import type { FrontstagePageCanvasRuntimeSourceState } from '../../lib/page-canvas/runtime-source';
import type { RestrictedBlockRuntimeHostSnapshot } from '../../lib/restricted-block-runtime-host';

function createPageContent(
  overrides: FrontstagePageContentFixtureOverrides = {}
): FrontstagePageContent {
  return createFrontstagePageContentFixture(overrides);
}

function createRuntimeSnapshot(
  overrides: Partial<RestrictedBlockRuntimeHostSnapshot> = {}
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status: 'ready',
    requestId: 'restricted-block:ready:ready-code',
    blockId: 'ready',
    schemaValidationOptions: {
      maxDepth: 8,
      maxNodes: 250,

      allowedEvents: [],
      allowedDataPermissions: []
    },
    logs: [],
    effects: [],
    rejections: [],
    ...overrides
  };
}

describe('PageCanvas runtime preview UI', () => {
  test('renders a restricted runtime preview from a synthetic session snapshot while preserving canvas state', () => {
    const runtimeSourceState = {
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      sources: [
        {
          status: 'ready',
          blockId: 'ready',
          sourceIndex: 0,
          slotIndex: 0,
          codeRef: 'ready-code'
        }
      ]
    } as FrontstagePageCanvasRuntimeSourceState;
    const runtimeRunPlanState = {
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      items: [
        {
          status: 'run_plan_ready',
          blockId: 'ready',
          sourceBlockId: 'ready',
          codeRef: 'ready-code',
          sourceCodeRef: 'ready-code',
          order: 0,
          sourceIndex: 0,
          slotIndex: 0,
          renderMode: 'restricted_js_block',
          canEnterRestrictedJsRuntime: true,
          runtimeKind: 'iframe',
          runtimeEntry: 'blocks/ready.js',
          contributionCode: 'official.ready',
          sourceStatus: 'ready',
          source_sha256: 'ready-source-sha256',
          catalogId: 'official:ready',
          runPlan: {
            ok: true,
            request: {
              requestId: 'restricted-block:ready:ready-code',
              blockId: 'ready',
              program: {
                kind: 'source',
                source: 'export default {}'
              },
              props: {},
              state: {},
              contextSnapshot: {},
              limits: {
                timeoutMs: 1000,
                maxRenderDepth: 8,
                maxRenderNodes: 250
              }
            },
            schemaValidationOptions: {
              maxDepth: 8,
              maxNodes: 250,

              allowedEvents: [],
              allowedDataPermissions: []
            },
            mediatorPolicy: {
              allowedEvents: [],
              maxEventChainDepth: 4
            }
          }
        }
      ]
    } as FrontstagePageCanvasRuntimeRunPlanState;
    const runtimeSessionEntries = [
      {
        status: 'ready',
        blockId: 'ready',
        sourceBlockId: 'ready',
        codeRef: 'ready-code',
        sourceCodeRef: 'ready-code',
        sourceIndex: 0,
        slotIndex: 0,
        sourceStatus: 'ready',
        runPlanStatus: 'run_plan_ready',
        snapshot: createRuntimeSnapshot({
          status: 'ready',
          view: {
            primitive: 'Title',
            props: { children: 'Synthetic Runtime Preview' }
          }
        })
      }
    ] satisfies FrontstagePageCanvasRuntimeSessionEntry[];

    render(
      <PageCanvas
        runtimeSourceState={runtimeSourceState}
        runtimeRunPlanState={runtimeRunPlanState}
        runtimeSessionEntries={runtimeSessionEntries}
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'ready',
                  renderer_version: 'v1',
                  codeRef: 'ready-code',
                  contributionCode: 'official.ready',
                  runtime: { kind: 'iframe', entry: 'blocks/ready.js' },
                  layout: { order: 0, region: 'main' }
                }
              ]
            }
          }
        })}
      />
    );

    expect(
      within(screen.getByTestId('page-canvas-render-slots')).getByTestId(
        'block-slot-ready'
      )
    ).toBeInTheDocument();
    // Block renders its actual content (no more debug status tags)
    expect(
      screen.getByTestId('restricted-block-runtime-preview')
    ).toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: 'Synthetic Runtime Preview' })
    ).toBeInTheDocument();
  });

  test('keeps stable runtime placeholders for factory failures and skipped sessions without rendering previews', () => {
    const runtimeRunPlanState = {
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      items: [
        {
          status: 'run_plan_ready',
          blockId: 'factory-failed',
          sourceBlockId: 'factory-failed',
          codeRef: 'factory-failed-code',
          sourceCodeRef: 'factory-failed-code',
          order: 0,
          sourceIndex: 0,
          slotIndex: 0,
          renderMode: 'restricted_js_block',
          canEnterRestrictedJsRuntime: true,
          runtimeKind: 'iframe',
          runtimeEntry: 'blocks/factory-failed.js',
          contributionCode: 'official.factory-failed',
          sourceStatus: 'ready',
          source_sha256: 'factory-failed-source-sha256',
          catalogId: 'official:factory-failed',
          runPlan: {
            ok: true,
            request: {
              requestId: 'restricted-block:factory-failed:factory-failed-code',
              blockId: 'factory-failed',
              source: 'export default {}',
              props: {},
              state: {},
              contextSnapshot: {},
              limits: {
                timeoutMs: 1000,
                maxRenderDepth: 8,
                maxRenderNodes: 250
              }
            },
            schemaValidationOptions: {
              maxDepth: 8,
              maxNodes: 250,

              allowedEvents: [],
              allowedDataPermissions: []
            },
            mediatorPolicy: {
              allowedEvents: [],
              maxEventChainDepth: 4
            }
          }
        },
        {
          status: 'source_not_ready',
          blockId: 'loading',
          sourceBlockId: 'loading',
          codeRef: 'loading-code',
          sourceCodeRef: 'loading-code',
          order: 1,
          sourceIndex: 1,
          slotIndex: 1,
          renderMode: 'restricted_js_block',
          canEnterRestrictedJsRuntime: true,
          runtimeKind: 'iframe',
          runtimeEntry: 'blocks/loading.js',
          contributionCode: 'official.loading',
          sourceStatus: 'loading',
          reason: {
            code: 'source_not_ready',
            path: 'sources.1.status',
            message: 'waiting for source'
          }
        }
      ]
    } as FrontstagePageCanvasRuntimeRunPlanState;
    const runtimeSessionEntries = [
      {
        status: 'factory_failed',
        blockId: 'factory-failed',
        sourceBlockId: 'factory-failed',
        codeRef: 'factory-failed-code',
        sourceCodeRef: 'factory-failed-code',
        sourceIndex: 0,
        slotIndex: 0,
        sourceStatus: 'ready',
        runPlanStatus: 'run_plan_ready',
        message: 'worker unavailable',
        error: new Error('worker unavailable')
      },
      {
        status: 'skipped',
        blockId: 'loading',
        sourceBlockId: 'loading',
        codeRef: 'loading-code',
        sourceCodeRef: 'loading-code',
        sourceIndex: 1,
        slotIndex: 1,
        sourceStatus: 'loading',
        runPlanStatus: 'source_not_ready',
        skipReason: 'source_not_ready',
        message: 'waiting for source',
        path: 'sources.1.status'
      }
    ] satisfies FrontstagePageCanvasRuntimeSessionEntry[];

    render(
      <PageCanvas
        runtimeRunPlanState={runtimeRunPlanState}
        runtimeSessionEntries={runtimeSessionEntries}
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'factory-failed',
                  renderer_version: 'v1',
                  codeRef: 'factory-failed-code',
                  contributionCode: 'official.factory-failed',
                  runtime: {
                    kind: 'iframe',
                    entry: 'blocks/factory-failed.js'
                  },
                  layout: { order: 0, region: 'main' }
                },
                {
                  id: 'loading',
                  renderer_version: 'v1',
                  codeRef: 'loading-code',
                  contributionCode: 'official.loading',
                  runtime: { kind: 'iframe', entry: 'blocks/loading.js' },
                  layout: { order: 1, region: 'main' }
                }
              ]
            }
          }
        })}
      />
    );

    expect(
      screen.queryByTestId('restricted-block-runtime-preview')
    ).not.toBeInTheDocument();
    expect(screen.getByText('运行时预览不可用')).toBeInTheDocument();
    expect(screen.getByText('受限运行时会话创建失败。')).toBeInTheDocument();
    expect(screen.getByTestId('block-ui-loading-shell')).toBeInTheDocument();
    expect(screen.queryByText('区块跳过运行')).not.toBeInTheDocument();
    expect(screen.queryByText('worker unavailable')).not.toBeInTheDocument();
  });
});
