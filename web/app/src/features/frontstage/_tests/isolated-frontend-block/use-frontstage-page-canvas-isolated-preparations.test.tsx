/* eslint-disable testing-library/render-result-naming-convention */

import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { sha256Text } from '@1flowbase/page-runtime';

import { i18nText } from '../../../../shared/i18n/text';

import type { FrontstageBlockCatalogEntry } from '../../api/block-catalog';
import {
  createFrontstageIsolatedPreparationRequests,
  useFrontstagePageCanvasIsolatedPreparations
} from '../../hooks/use-frontstage-page-canvas-isolated-preparations';
import { normalizeFrontstageBlockCatalog } from '../../lib/block-catalog';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import {
  createFrontstageBlockRenderPlanItem,
  type FrontstagePageRenderPlan
} from '../../lib/page-canvas/render-plan';

const SOURCE = `globalThis.__oneflowbaseIsolatedBlock = {
  mount(root, props) { root.textContent = String(props.label); }
};`;

describe('FrontStagePage isolated contribution preparation', () => {
  test('joins an isolated render request to the exact graph-backed catalog binding', async () => {
    const renderPlan = isolatedRenderPlan('isolated-a');
    const catalogEntries = normalizeFrontstageBlockCatalog([
      isolatedCatalogEntry()
    ]).items;
    const fetchAsset = vi.fn(async () => new Response(SOURCE, { status: 200 }));
    const { result, rerender } = renderHook(
      ({ plan }) =>
        useFrontstagePageCanvasIsolatedPreparations({
          actorId: 'actor-1',
          actorWorkspaceId: 'workspace-1',
          workspaceId: 'workspace-1',
          renderPlan: plan,
          catalogEntries,
          fetchAsset
        }),
      { initialProps: { plan: renderPlan as FrontstagePageRenderPlan | null } }
    );

    await waitFor(() => expect(result.current.preparations).toHaveLength(1));
    expect(result.current.preparations[0]).toMatchObject({
      blockInstanceId: 'isolated-a',
      graphFingerprint: 'graph-fingerprint',
      runtimeKind: 'isolated',
      isolationRequirement: 'independent_realm',
      program: { props: { label: 'isolated-a' } }
    });
    expect(result.current.errorsByBlockId).toEqual({});
    expect(fetchAsset).toHaveBeenCalledOnce();

    rerender({ plan: null });
    await waitFor(() => expect(result.current.preparations).toEqual([]));
  });

  test('fails closed for a missing binding and aborts superseded asset fetches', async () => {
    const renderPlan = isolatedRenderPlan('isolated-a');
    const missing = renderHook(() =>
      useFrontstagePageCanvasIsolatedPreparations({
        actorId: 'actor-1',
        actorWorkspaceId: 'workspace-1',
        workspaceId: 'workspace-1',
        renderPlan,
        catalogEntries: []
      })
    );
    await waitFor(() =>
      expect(missing.result.current.errorsByBlockId['isolated-a']).toEqual(
        new Error(i18nText('frontstage', 'auto.runtime_preview_unavailable'))
      )
    );

    let fetchSignal: AbortSignal | null = null;
    const pendingFetch: typeof fetch = (_input, init) => {
      fetchSignal = init?.signal ?? null;
      return new Promise<Response>((_resolve, reject) => {
        fetchSignal?.addEventListener('abort', () =>
          reject(new DOMException('aborted', 'AbortError'))
        );
      });
    };
    const catalogEntries = normalizeFrontstageBlockCatalog([
      isolatedCatalogEntry()
    ]).items;
    const pending = renderHook(
      ({ plan }) =>
        useFrontstagePageCanvasIsolatedPreparations({
          actorId: 'actor-1',
          actorWorkspaceId: 'workspace-1',
          workspaceId: 'workspace-1',
          renderPlan: plan,
          catalogEntries,
          fetchAsset: pendingFetch
        }),
      { initialProps: { plan: renderPlan as FrontstagePageRenderPlan | null } }
    );
    await waitFor(() => expect(fetchSignal).not.toBeNull());
    pending.rerender({ plan: null });
    expect((fetchSignal as AbortSignal | null)?.aborted).toBe(true);
  });

  test('does not create isolated requests from trusted Native items', () => {
    const native = isolatedBlock('native-a');
    native.runtime = {
      kind: 'native_react',
      entry: '@acme/native-block',
      hint: 'native_react'
    };
    native.codeRef = 'native-code';
    native.sourceCodeRef = 'native-code';
    const renderPlan: FrontstagePageRenderPlan = {
      pageId: 'page-1',
      rootUid: 'root-1',
      isEmpty: false,
      diagnostics: [],
      items: [createFrontstageBlockRenderPlanItem(native)]
    };

    expect(
      createFrontstageIsolatedPreparationRequests({
        workspaceId: 'workspace-1',
        renderPlan
      })
    ).toEqual([]);
  });
});

function isolatedRenderPlan(blockId: string): FrontstagePageRenderPlan {
  return {
    pageId: 'page-1',
    rootUid: 'root-1',
    isEmpty: false,
    diagnostics: [],
    items: [createFrontstageBlockRenderPlanItem(isolatedBlock(blockId))]
  };
}

function isolatedBlock(id: string): FrontstageBlockInstance {
  return {
    id,
    rendererVersion: 'v1',
    sourceId: id,
    codeRef: '',
    sourceCodeRef: null,
    catalog: {
      installationId: 'installation-1',
      providerCode: 'acme'
    },
    contribution: {
      pluginId: 'acme.blocks',
      pluginVersion: '1.0.0',
      code: 'isolated-chart'
    },
    props: { label: id },
    ports: { inputs: [], outputs: [] },
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0 },
    order: 0,
    runtime: {
      kind: 'isolated_iframe',
      entry: '@acme/isolated-chart',
      hint: 'isolated_iframe'
    }
  };
}

function isolatedCatalogEntry(): FrontstageBlockCatalogEntry {
  const digest = sha256Text(SOURCE);
  return {
    installation_id: 'installation-1',
    provider_code: 'acme',
    plugin_id: 'acme.blocks',
    plugin_version: '1.0.0',
    contribution_code: 'isolated-chart',
    title: 'Isolated chart',
    runtime: 'isolated_iframe',
    entry: '@acme/isolated-chart',
    isolated_entry_asset: {
      media_type: 'text/javascript; charset=utf-8',
      sha256: digest,
      url: `/api/console/frontstage/component-module-assets/${digest}`,
      integrity: 'verified_sha256'
    },
    context_contract: { primitives: [], input_schema: { type: 'object' } },
    permissions: { network: 'none', storage: 'none', secrets: 'none' },
    ui_capabilities: [],
    frontend_contribution_id: 'frontend-block.installation-1.isolated-chart',
    frontend_block_id: 'installation-1:isolated-chart',
    frontend_block_version: '1.0.0',
    runtime_kind: 'isolated',
    execution_kind: 'ui_mount',
    isolation_requirement: 'independent_realm',
    requested_permissions: ['frontend-block.ui-mount.isolated-realm'],
    granted_permissions: ['frontend-block.ui-mount.isolated-realm'],
    workspace_id: 'workspace-1',
    lifecycle_kind: 'workspace_assignment',
    graph_fingerprint: 'graph-fingerprint',
    provenance: {
      module_id: '1flowbase.boot.core',
      module_version: '1',
      module_kind: 'boot_core'
    },
    disable_reason: null
  };
}
