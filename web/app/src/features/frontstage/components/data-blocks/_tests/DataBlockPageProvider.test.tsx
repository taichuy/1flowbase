import { useEffect } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ConsoleFrontstageBlockNode } from '@1flowbase/api-client';
import { createFrontstagePageContentFixture } from '../../../_tests/frontstage-page-content-fixtures';
import { normalizeFrontstageBlockCatalog } from '../../../lib/block-catalog';
import {
  useDataBlockHost,
  type DataBlockHost
} from '../../../lib/data-blocks/host-context';
import {
  createFormBlockConfig,
  createTableBlockConfig,
  FORM_BLOCK_CONTRIBUTION,
  type DataBlockAction,
  type DataBlockConfig,
  type DataBlockRelation
} from '../../../lib/data-blocks/types';
import type { FrontstageBlockRenderPlanItem } from '../../../lib/page-canvas/render-plan';
import {
  DataBlockPageProvider,
  DataBlockSlotProvider
} from '../DataBlockPageProvider';

const mocks = vi.hoisted(() => ({
  fetchNode: vi.fn(),
  fetchRoots: vi.fn(),
  fetchCode: vi.fn(),
  saveCode: vi.fn(),
  create: vi.fn(),
  update: vi.fn(),
  mutations: vi.fn(),
  catalog: vi.fn()
}));
vi.mock('../../../api/block-tree', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../api/block-tree')>()),
  fetchFrontstageBlockNode: mocks.fetchNode,
  fetchFrontstageBlockRoots: mocks.fetchRoots,
  fetchFrontstageBlockNodeCode: mocks.fetchCode,
  saveFrontstageBlockNodeCode: mocks.saveCode
}));
vi.mock('../../../hooks/use-frontstage-block-tree-mutations', () => ({
  useFrontstageBlockTreeMutations: mocks.mutations
}));
vi.mock('../../../hooks/use-frontstage-block-catalog', () => ({
  useFrontstageBlockCatalog: mocks.catalog
}));

const binding = {
  model_id: 'orders-model',
  model_code: 'orders',
  data_source_id: 'crm-source'
};
const relation: DataBlockRelation = {
  field_code: 'customer_id',
  parent_model_code: 'customers',
  parent_record_input: 'customer_record'
};
const sourceCode =
  'export default function CustomTable() { return <p>Keep authored code</p>; }';
const formTemplate =
  'export default function FormTemplate() { return <p>Catalog form template</p>; }';
const customInput = {
  name: 'custom_context',
  schema: { type: 'object' },
  source: { block_id: 'context-block', output: 'context', scope: 'page' }
};
const customOutput = { name: 'custom_event', schema: { type: 'string' } };
const recordSource = {
  block_id: 'selection-block',
  output: 'selected_id',
  scope: 'page'
};
const catalog = normalizeFrontstageBlockCatalog([
  {
    installation_id: 'builtin-installation',
    provider_code: '1flowbase',
    plugin_id: 'builtin-frontstage',
    plugin_version: '1.0.0',
    contribution_code: FORM_BLOCK_CONTRIBUTION,
    title: 'Form',
    runtime: 'native_react',
    entry: 'index.js',
    code_modules: [],
    code_template: formTemplate,
    code_template_version: '1',
    code_template_language: 'tsx',
    context_contract: { primitives: [], input_schema: {} },
    permissions: { network: 'none', storage: 'none', secrets: 'none' },
    ui_capabilities: [],
    frontend_contribution_id: 'form-contribution',
    frontend_block_id: 'form-template',
    frontend_block_version: '1.0.0',
    runtime_kind: 'trusted_native',
    execution_kind: 'ui_mount',
    isolation_requirement: 'trusted_host_realm',
    requested_permissions: [],
    granted_permissions: [],
    workspace_id: 'workspace-1',
    lifecycle_kind: 'workspace_assignment',
    graph_fingerprint: 'catalog-fixture',
    provenance: {
      module_id: 'builtin-frontstage',
      module_version: '1.0.0',
      module_kind: 'trusted_host'
    },
    disable_reason: null
  }
]).items;

function node(
  block_id: string,
  config: DataBlockConfig,
  parent_block_id: string | null = null
): ConsoleFrontstageBlockNode {
  return {
    block_id,
    workspace_id: 'workspace-1',
    page_id: 'page-1',
    tab_id: 'tab-1',
    parent_block_id,
    rank: '001000',
    presentation: parent_block_id ? 'drawer' : 'page',
    title: block_id,
    description: null,
    schema_version: 1,
    created_at: '2026-11-06T00:00:00Z',
    updated_at: '2026-11-06T00:00:00Z',
    code_ref: `${block_id}-source`,
    input_mapping: { custom_context: 'context' },
    output_mapping: { custom_event: 'event' },
    runtime_descriptor: {
      codeRef: `${block_id}-source`,
      runtime: {
        kind: 'native_react',
        entry: 'custom.js',
        custom_option: true
      },
      'x-layout': { order: 3, width: 12 },
      custom_extension: { retained: true },
      props: {
        data_block: config,
        custom_title: 'Authored title',
        custom_options: { keep: true }
      },
      ports: {
        inputs: [
          customInput,
          {
            name: 'record_id',
            schema: { type: 'string' },
            source: recordSource
          },
          ...(config.relation
            ? [
                {
                  name: config.relation.parent_record_input,
                  schema: { type: 'string' }
                }
              ]
            : [])
        ],
        outputs: [customOutput]
      }
    }
  };
}

function renderItem(config: DataBlockConfig): FrontstageBlockRenderPlanItem {
  return {
    blockId: 'table-block',
    sourceBlockId: null,
    codeRef: 'table-block-source',
    sourceCodeRef: null,
    rendererVersion: '1',
    sourceIndex: 0,
    order: 0,
    renderMode: 'native_react',
    canPrepareNativeReact: true,
    canMountIsolatedIframe: false,
    fallbackReasons: [],
    catalog: {
      providerCode: '1flowbase',
      installationId: 'builtin-installation'
    },
    contribution: {
      pluginId: 'builtin-frontstage',
      pluginVersion: '1.0.0',
      code: 'frontstage.table-block'
    },
    runtime: { kind: 'native_react', entry: 'custom.js', hint: 'native_react' },
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0 },
    props: { data_block: config, custom_title: 'Stale render title' }
  };
}

function Probe({ capture }: { capture(host: DataBlockHost | null): void }) {
  const host = useDataBlockHost();
  useEffect(() => {
    capture(host);
  }, [capture, host]);
  return <span>{host?.block_id ?? 'no-host'}</span>;
}

async function setup(isDesignMode = true) {
  const config = createTableBlockConfig(binding);
  const currentNode = node('table-block', config);
  const otherRoot = node('other-root', createFormBlockConfig(binding));
  const nodes = new Map([
    [currentNode.block_id, currentNode],
    [otherRoot.block_id, otherRoot]
  ]);
  mocks.fetchNode.mockImplementation(
    async (_workspace: string, _page: string, id: string) => {
      const value = nodes.get(id);
      if (!value) throw new Error(`Missing fixture node: ${id}`);
      return value;
    }
  );
  mocks.fetchRoots.mockResolvedValue([currentNode, otherRoot]);
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } }
  });
  let host: DataBlockHost | null = null;
  const capture = (value: DataBlockHost | null) => {
    host = value;
  };
  render(
    <QueryClientProvider client={client}>
      <DataBlockPageProvider
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
      >
        <DataBlockSlotProvider
          item={renderItem(config)}
          isDesignMode={isDesignMode}
          pageContent={createFrontstagePageContentFixture()}
        >
          <Probe capture={capture} />
        </DataBlockSlotProvider>
      </DataBlockPageProvider>
    </QueryClientProvider>
  );
  const getHost = () => {
    if (!host) throw new Error('Data block host was not provided');
    return host;
  };
  await waitFor(() =>
    expect(getHost().targets).toEqual([
      { block_id: 'other-root', title: 'other-root', kind: 'form' }
    ])
  );
  return { config, nodes, getHost };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.create.mockResolvedValue({ block_id: 'created-form' });
  mocks.update.mockResolvedValue(undefined);
  mocks.fetchCode.mockResolvedValue({ source_code: sourceCode });
  mocks.mutations.mockReturnValue({
    create: { mutateAsync: mocks.create },
    update: { mutateAsync: mocks.update },
    saveCode: { mutateAsync: mocks.saveCode }
  });
  mocks.catalog.mockReturnValue({
    items: catalog,
    loading: false,
    error: null,
    diagnostics: []
  });
});

describe('DataBlockPageProvider design persistence', () => {
  it('saves configuration against the latest descriptor without rewriting authored source, props, or ports', async () => {
    const { config, nodes, getHost } = await setup();
    const latest = node('table-block', config);
    const latestDescriptor = latest.runtime_descriptor as Record<
      string,
      unknown
    >;
    latest.runtime_descriptor = {
      ...latestDescriptor,
      server_extension: 'latest-value'
    };
    nodes.set('table-block', latest);
    const next = { ...config, title: 'New table title', relation };
    await act(async () => {
      await getHost().saveConfig(next);
    });
    expect(mocks.update).toHaveBeenCalledExactlyOnceWith({
      block_id: 'table-block',
      input: {
        runtime_descriptor: expect.objectContaining({
          codeRef: 'table-block-source',
          runtime: {
            kind: 'native_react',
            entry: 'custom.js',
            custom_option: true
          },
          'x-layout': { order: 3, width: 12 },
          custom_extension: { retained: true },
          server_extension: 'latest-value',
          props: {
            data_block: next,
            custom_title: 'Authored title',
            custom_options: { keep: true }
          },
          ports: {
            inputs: expect.arrayContaining([
              customInput,
              {
                name: 'record_id',
                schema: { type: 'string' },
                source: recordSource
              },
              { name: 'customer_record', schema: { type: 'string' } }
            ]),
            outputs: expect.arrayContaining([
              customOutput,
              { name: 'record_id', schema: { type: 'string' } },
              {
                name: 'selected_record_ids',
                schema: { type: 'array', items: { type: 'string' } }
              }
            ])
          }
        })
      }
    });
    expect(mocks.fetchCode).not.toHaveBeenCalled();
    expect(mocks.saveCode).not.toHaveBeenCalled();
    expect(mocks.create).not.toHaveBeenCalled();
    expect(getHost().saving).toBe(false);
  });

  it.each([
    { mode: 'create', kind: 'create', presentation: 'modal', size: 'large' },
    { mode: 'edit', kind: 'edit', presentation: 'drawer', size: 'small' },
    { mode: 'view', kind: 'view', presentation: 'page', size: 'middle' }
  ] as const)(
    'creates an actual $mode child form with its catalog template and explicit data context',
    async ({ mode, kind, presentation, size }) => {
      const { getHost } = await setup();
      const action: DataBlockAction = {
        id: `action-${mode}`,
        kind,
        enabled: true,
        title: 'Customer orders',
        presentation,
        size
      };
      let prepared: DataBlockAction | undefined;
      await act(async () => {
        prepared = await getHost().prepareAction(
          action,
          binding,
          mode,
          relation
        );
      });
      expect(prepared).toEqual({ ...action, target_block_id: 'created-form' });
      expect(mocks.create).toHaveBeenCalledExactlyOnceWith(
        expect.objectContaining({
          tab_id: 'tab-1',
          title: 'Customer orders',
          parent_block_id: 'table-block',
          presentation,
          before_block_id: null,
          after_block_id: null,
          source_code: formTemplate,
          runtime_descriptor: expect.objectContaining({
            contribution: expect.objectContaining({
              code: FORM_BLOCK_CONTRIBUTION
            }),
            props: {
              data_block: expect.objectContaining({
                kind: 'form',
                initialized: false,
                mode,
                binding,
                relation,
                source_block_id: 'table-block',
                source_model_code: 'orders',
                surface_size: size
              })
            },
            ports: {
              inputs: expect.arrayContaining([
                { name: 'record_id', schema: { type: 'string' } },
                { name: 'customer_record', schema: { type: 'string' } }
              ]),
              outputs: expect.arrayContaining([
                { name: 'saved_record_id', schema: { type: 'string' } },
                { name: 'save_revision', schema: { type: 'integer' } },
                { name: 'saved_record', schema: { type: 'object' } }
              ])
            }
          })
        })
      );
      expect(mocks.update).not.toHaveBeenCalled();
      expect(mocks.saveCode).not.toHaveBeenCalled();
    }
  );

  it('updates the existing child relation and surface size while retaining its authored form configuration', async () => {
    const { nodes, getHost } = await setup();
    const childConfig = {
      ...createFormBlockConfig(binding, 'edit'),
      title: 'Custom edit form',
      source_block_id: 'table-block',
      source_model_code: 'orders',
      items: [
        {
          id: 'amount',
          kind: 'field' as const,
          field_code: 'amount',
          row: 'amount',
          span: 12
        }
      ],
      relation: { ...relation, parent_record_input: 'old_customer_record' },
      surface_size: 'small' as const
    };
    nodes.set(
      'existing-form',
      node('existing-form', childConfig, 'table-block')
    );
    const action: DataBlockAction = {
      id: 'edit',
      kind: 'edit',
      enabled: true,
      target_block_id: 'existing-form',
      presentation: 'modal',
      size: 'large'
    };
    let prepared: DataBlockAction | undefined;
    await act(async () => {
      prepared = await getHost().prepareAction(
        action,
        binding,
        'edit',
        relation
      );
    });
    expect(prepared).toEqual(action);
    expect(mocks.create).not.toHaveBeenCalled();
    expect(mocks.update).toHaveBeenCalledExactlyOnceWith({
      block_id: 'existing-form',
      input: {
        presentation: 'modal',
        runtime_descriptor: expect.objectContaining({
          codeRef: 'existing-form-source',
          props: {
            custom_title: 'Authored title',
            custom_options: { keep: true },
            data_block: { ...childConfig, relation, surface_size: 'large' }
          },
          ports: {
            inputs: expect.arrayContaining([
              customInput,
              {
                name: 'record_id',
                schema: { type: 'string' },
                source: recordSource
              },
              { name: 'customer_record', schema: { type: 'string' } }
            ]),
            outputs: expect.arrayContaining([customOutput])
          }
        })
      }
    });
    const ports = mocks.update.mock.calls[0][0].input.runtime_descriptor.ports;
    expect(ports.inputs).not.toContainEqual(
      expect.objectContaining({ name: 'old_customer_record' })
    );
    expect(mocks.saveCode).not.toHaveBeenCalled();
  });

  it('preserves a child form bound to another model when the source relation changes', async () => {
    const { nodes, getHost } = await setup();
    const independentBinding = {
      model_id: 'notes-model',
      model_code: 'notes',
      data_source_id: null
    };
    const independentRelation = {
      field_code: 'author',
      parent_model_code: 'users',
      parent_record_input: 'author_id'
    };
    const childConfig = {
      ...createFormBlockConfig(independentBinding),
      title: 'Independent notes',
      relation: independentRelation,
      surface_size: 'small' as const,
      source_block_id: 'table-block',
      source_model_code: 'orders',
      items: [
        {
          id: 'body',
          kind: 'field' as const,
          field_code: 'body',
          row: 'body',
          span: 24
        }
      ]
    };
    nodes.set(
      'other-model-form',
      node('other-model-form', childConfig, 'table-block')
    );
    const action: DataBlockAction = {
      id: 'create',
      kind: 'create',
      enabled: true,
      target_block_id: 'other-model-form',
      presentation: 'modal',
      size: 'large'
    };
    await act(async () => {
      await getHost().prepareAction(action, binding, 'create', relation);
    });
    expect(mocks.update).toHaveBeenCalledExactlyOnceWith({
      block_id: 'other-model-form',
      input: {
        presentation: 'modal',
        runtime_descriptor: expect.objectContaining({
          props: expect.objectContaining({
            data_block: { ...childConfig, surface_size: 'large' }
          })
        })
      }
    });
    expect(mocks.create).not.toHaveBeenCalled();
  });

  it('does not rewrite an independent target block as a child form', async () => {
    const { nodes, getHost } = await setup();
    nodes.set('independent-form', {
      ...node('independent-form', createFormBlockConfig(binding)),
      presentation: 'page'
    });
    const action: DataBlockAction = {
      id: 'open',
      kind: 'open',
      enabled: true,
      target_block_id: 'independent-form',
      presentation: 'modal',
      size: 'large'
    };
    let prepared: DataBlockAction | undefined;
    await act(async () => {
      prepared = await getHost().prepareAction(
        action,
        binding,
        'view',
        relation
      );
    });
    expect(prepared).toEqual({ ...action, presentation: 'page' });
    expect(mocks.update).not.toHaveBeenCalled();
    expect(mocks.create).not.toHaveBeenCalled();
  });

  it('rejects all design writes in runtime mode, including updates of an existing child', async () => {
    const { config, getHost } = await setup(false);
    expect(getHost().design_mode).toBe(false);
    const readsBefore = mocks.fetchNode.mock.calls.length;
    await act(async () => {
      await expect(
        getHost().saveConfig({ ...config, title: 'Forbidden' })
      ).rejects.toThrow();
      await expect(
        getHost().prepareAction(
          { id: 'new', kind: 'create', enabled: true },
          binding,
          'create',
          relation
        )
      ).rejects.toThrow();
      await expect(
        getHost().prepareAction(
          {
            id: 'existing',
            kind: 'edit',
            enabled: true,
            target_block_id: 'existing-form'
          },
          binding,
          'edit',
          relation
        )
      ).rejects.toThrow();
    });
    expect(mocks.catalog).toHaveBeenLastCalledWith({ workspaceId: null });
    expect(mocks.fetchNode).toHaveBeenCalledTimes(readsBefore);
    expect(mocks.create).not.toHaveBeenCalled();
    expect(mocks.update).not.toHaveBeenCalled();
    expect(mocks.saveCode).not.toHaveBeenCalled();
  });
});
