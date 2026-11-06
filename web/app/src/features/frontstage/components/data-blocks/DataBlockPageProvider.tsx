import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode
} from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { ConsoleFrontstageBlockNode } from '@1flowbase/api-client';
import type { FrontstagePageContent } from '../../api/page-content';
import {
  fetchFrontstageBlockNode,
  fetchFrontstageBlockRoots,
  frontstageBlockTreeQueryKeys
} from '../../api/block-tree';
import { useFrontstageBlockTreeMutations } from '../../hooks/use-frontstage-block-tree-mutations';
import { useFrontstageBlockCatalog } from '../../hooks/use-frontstage-block-catalog';
import type { FrontstageBlockRenderPlanItem } from '../../lib/page-canvas/render-plan';
import {
  DataBlockHostContext,
  type DataBlockHost
} from '../../lib/data-blocks/host-context';
import {
  createFormBlockConfig,
  FORM_BLOCK_CONTRIBUTION,
  readDataBlockConfig,
  type DataBlockConfig
} from '../../lib/data-blocks/types';
import {
  dataBlockPorts,
  mergeDataBlockPorts
} from '../../lib/data-blocks/ports';
import { createCatalogBlockInput } from '../../pages/frontstage-page/block-catalog-helpers';
import { appendFrontstageBlock } from '../../lib/block-composition';
import { createFrontstageBlockRuntimeDescriptor } from '../../lib/page-document';
import { i18nText } from '../../../../shared/i18n/text';

interface PageHost {
  workspaceId: string;
  pageId: string | null;
  tabId?: string;
  onDragActivityChange?(blockId: string, active: boolean): void;
  navigate?(
    blockId: string | null,
    options?: { inputs?: Readonly<Record<string, string>> }
  ): void;
}
const PageContext = createContext<PageHost | null>(null);

export function DataBlockPageProvider({
  children,
  ...value
}: PageHost & { children: ReactNode }) {
  const context = useMemo(
    () => value,
    [
      value.workspaceId,
      value.pageId,
      value.tabId,
      value.navigate,
      value.onDragActivityChange
    ]
  );
  return (
    <PageContext.Provider value={context}>{children}</PageContext.Provider>
  );
}

function descriptor(node: ConsoleFrontstageBlockNode): Record<string, unknown> {
  return node.runtime_descriptor &&
    typeof node.runtime_descriptor === 'object' &&
    !Array.isArray(node.runtime_descriptor)
    ? (node.runtime_descriptor as Record<string, unknown>)
    : {};
}

function descriptorWithConfig(
  node: ConsoleFrontstageBlockNode,
  config: DataBlockConfig
): Record<string, unknown> {
  const current = descriptor(node);
  return {
    ...current,
    props: {
      ...(current.props && typeof current.props === 'object'
        ? current.props
        : {}),
      data_block: config
    },
    ports: mergeDataBlockPorts(
      current.ports,
      readDataBlockConfig((current.props ?? {}) as Record<string, unknown>),
      config
    )
  };
}

export function DataBlockSlotProvider({
  item,
  isDesignMode,
  pageContent,
  children
}: {
  item: FrontstageBlockRenderPlanItem;
  isDesignMode: boolean;
  pageContent?: FrontstagePageContent;
  children: ReactNode;
}) {
  const page = useContext(PageContext);
  if (!page?.pageId || !page.tabId || !readDataBlockConfig(item.props))
    return children;
  return (
    <DataBlockSlotHost
      page={{ ...page, pageId: page.pageId, tabId: page.tabId }}
      item={item}
      isDesignMode={isDesignMode}
      pageContent={pageContent}
    >
      {children}
    </DataBlockSlotHost>
  );
}

function DataBlockSlotHost({
  page,
  item,
  isDesignMode,
  children,
  pageContent
}: {
  page: PageHost & { pageId: string; tabId: string };
  item: FrontstageBlockRenderPlanItem;
  isDesignMode: boolean;
  children: ReactNode;
  pageContent?: FrontstagePageContent;
}) {
  const { workspaceId, pageId, tabId } = page;
  const setDragActive = useCallback(
    (active: boolean) => page.onDragActivityChange?.(item.blockId, active),
    [page.onDragActivityChange, item.blockId]
  );
  const mutations = useFrontstageBlockTreeMutations(workspaceId, pageId);
  const catalog = useFrontstageBlockCatalog({
    workspaceId: isDesignMode ? workspaceId : null
  });
  const queryClient = useQueryClient();
  const [saving, setSaving] = useState(false);
  const node = useQuery({
    queryKey: frontstageBlockTreeQueryKeys.block(
      workspaceId,
      pageId,
      item.blockId
    ),
    queryFn: () => fetchFrontstageBlockNode(workspaceId, pageId, item.blockId),
    retry: false
  });
  const roots = useQuery({
    queryKey: frontstageBlockTreeQueryKeys.roots(workspaceId, pageId, {
      tab_id: tabId
    }),
    queryFn: () =>
      fetchFrontstageBlockRoots(workspaceId, pageId, { tab_id: tabId }),
    retry: false
  });
  const invalidate = () =>
    queryClient.invalidateQueries({
      queryKey: frontstageBlockTreeQueryKeys.page(workspaceId, pageId)
    });
  const host: DataBlockHost = {
    block_id: item.blockId,
    design_mode: isDesignMode,
    setDragActive,
    saving,
    targets: (roots.data ?? [])
      .filter((candidate) => candidate.block_id !== item.blockId)
      .map((candidate) => {
        const config = readDataBlockConfig(
          (descriptor(candidate).props ?? {}) as Record<string, unknown>
        );
        return {
          block_id: candidate.block_id,
          title: candidate.title || config?.title || candidate.block_id,
          kind: config?.kind ?? 'code'
        };
      }),
    async saveConfig(config) {
      if (!isDesignMode || saving)
        throw new Error(
          i18nText('frontstage', 'data_blocks.common.design_unavailable')
        );
      setSaving(true);
      try {
        const current = await fetchFrontstageBlockNode(
          workspaceId,
          pageId,
          item.blockId
        );
        await mutations.update.mutateAsync({
          block_id: item.blockId,
          input: { runtime_descriptor: descriptorWithConfig(current, config) }
        });
        await invalidate();
      } finally {
        setSaving(false);
      }
    },
    async prepareAction(action, binding, mode, relation) {
      if (!isDesignMode)
        throw new Error(
          i18nText('frontstage', 'data_blocks.common.design_unavailable')
        );
      if (action.target_block_id) {
        const existing = await fetchFrontstageBlockNode(
          workspaceId,
          pageId,
          action.target_block_id
        );
        const config = readDataBlockConfig(
          (descriptor(existing).props ?? {}) as Record<string, unknown>
        );
        if (
          existing.parent_block_id === item.blockId &&
          config?.kind === 'form'
        ) {
          await mutations.update.mutateAsync({
            block_id: existing.block_id,
            input: {
              presentation: action.presentation ?? 'drawer',
              runtime_descriptor: descriptorWithConfig(existing, {
                ...config,
                relation:
                  config.binding.model_id === binding.model_id &&
                  config.binding.model_code === binding.model_code &&
                  config.binding.data_source_id === binding.data_source_id
                    ? relation
                    : config.relation,
                surface_size: action.size ?? 'middle'
              })
            }
          });
          await invalidate();
          return action;
        }
        return {
          ...action,
          presentation:
            existing.presentation === 'inline' ? 'page' : existing.presentation
        };
      }
      const entry = catalog.items.find(
        (candidate) => candidate.contributionCode === FORM_BLOCK_CONTRIBUTION
      );
      if (!entry?.codeCapabilities?.template || !pageContent)
        throw new Error(
          i18nText('frontstage', 'data_blocks.common.form_template_unavailable')
        );
      const input = createCatalogBlockInput(entry, 0);
      const formConfig = {
        ...createFormBlockConfig(binding, mode),
        initialized: false,
        relation,
        source_block_id: item.blockId,
        source_model_code: binding.model_code,
        surface_size: action.size ?? ('middle' as const)
      };
      input.props = { data_block: formConfig };
      const state = appendFrontstageBlock(
        {
          document: {
            page: pageContent.page,
            rootUid: pageContent.document.rootUid,
            layoutMode: 'auto',
            blocks: [],
            isEmpty: true,
            diagnostics: []
          },
          selectedBlockId: null
        },
        input
      );
      const block = state.document.blocks[0]!;
      block.ports = dataBlockPorts(formConfig);
      const created = await mutations.create.mutateAsync({
        tab_id: tabId,
        title:
          action.title ||
          (mode === 'create'
            ? i18nText('frontstage', 'data_blocks.common.create')
            : mode === 'edit'
              ? i18nText('frontstage', 'auto.edit')
              : i18nText('frontstage', 'data_blocks.common.view')),
        description: '',
        presentation: action.presentation ?? 'drawer',
        parent_block_id: item.blockId,
        before_block_id: null,
        after_block_id: null,
        source_code: entry.codeCapabilities.template.source,
        runtime_descriptor: createFrontstageBlockRuntimeDescriptor(block)
      });
      await invalidate();
      return { ...action, target_block_id: created.block_id };
    },
    openAction(action, inputs) {
      if (!action.target_block_id || !page.navigate)
        throw new Error(
          i18nText('frontstage', 'data_blocks.common.target_unavailable')
        );
      page.navigate(action.target_block_id, {
        inputs: { ...inputs, source_block_id: item.blockId }
      });
    },
    close() {
      const parentId = node.data?.parent_block_id;
      const rootParent = (roots.data ?? []).some(
        (candidate) => candidate.block_id === parentId
      );
      page.navigate?.(rootParent ? null : (parentId ?? null));
    }
  };
  return (
    <DataBlockHostContext.Provider value={host}>
      {children}
    </DataBlockHostContext.Provider>
  );
}
