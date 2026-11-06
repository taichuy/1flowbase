import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Button, Dropdown, Input, type MenuProps } from 'antd';
import {
  fetchConsoleDataModels,
  fetchConsoleDataSources
} from '@1flowbase/api-client';
import { i18nText } from '../../../../shared/i18n/text';
import { useAuthStore } from '../../../../state/auth-store';
import { getFrontstageApiBaseUrl } from '../../api/page-tree';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import {
  FORM_BLOCK_CONTRIBUTION,
  TABLE_BLOCK_CONTRIBUTION,
  type DataBlockBinding
} from '../../lib/data-blocks/types';
import { FRONTSTAGE_DESIGN_BLUE } from '../../lib/design-mode-theme';

export function CreateDataBlockMenu({
  workspaceId,
  entries,
  disabled,
  loading,
  error,
  onRetry,
  onCreate
}: {
  workspaceId: string;
  entries: readonly NormalizedFrontstageBlockCatalogEntry[];
  disabled: boolean;
  loading: boolean;
  error?: Error | null;
  onRetry?(): void;
  onCreate(
    entry: NormalizedFrontstageBlockCatalogEntry,
    binding?: DataBlockBinding
  ): void | Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const actor = useAuthStore((state) => state.actor);
  const models = useQuery({
    queryKey: ['frontstage-design-models', workspaceId, actor?.id],
    queryFn: () => fetchConsoleDataModels({}, getFrontstageApiBaseUrl()),
    enabled: open,
    retry: false
  });
  const sources = useQuery({
    queryKey: ['frontstage-design-sources', workspaceId, actor?.id],
    queryFn: () => fetchConsoleDataSources(getFrontstageApiBaseUrl()),
    enabled: open,
    retry: false
  });
  const dataEntries = entries.filter(
    (entry) =>
      entry.contributionCode === TABLE_BLOCK_CONTRIBUTION ||
      entry.contributionCode === FORM_BLOCK_CONTRIBUTION
  );
  const codeEntries = entries.filter((entry) => !dataEntries.includes(entry));
  const choose = (
    entry: NormalizedFrontstageBlockCatalogEntry,
    binding?: DataBlockBinding
  ) => {
    setOpen(false);
    onCreate(entry, binding);
  };
  const items: MenuProps['items'] = [];
  if (dataEntries.length) {
    items.push({
      key: 'data',
      type: 'group',
      label: i18nText('frontstage', 'data_blocks.create.data'),
      children: dataEntries.map((entry) => {
        const available = (models.data ?? []).filter(
          (model) =>
            model.status === 'published' &&
            model.capabilities.record.can_list &&
            `${model.title} ${model.code}`
              .toLowerCase()
              .includes(search.toLowerCase())
        );
        const sourceGroups = (sources.data ?? [])
          .map((source) => ({
            key: `${entry.id}:${source.id}`,
            label: source.display_name,
            children: available
              .filter((model) => (model.data_source_id ?? 'main') === source.id)
              .map((model) => ({
                key: `${entry.id}:${model.id}`,
                label: model.title,
                onClick: () =>
                  choose(entry, {
                    model_id: model.id,
                    model_code: model.code,
                    data_source_id: model.data_source_id
                  })
              }))
          }))
          .filter((group) => group.children.length);
        const children: MenuProps['items'] = [
          {
            key: `${entry.id}:search`,
            label: (
              <Input
                aria-label={i18nText('frontstage', 'data_blocks.create.search')}
                placeholder={i18nText(
                  'frontstage',
                  'data_blocks.create.search'
                )}
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                onKeyDown={(event) => event.stopPropagation()}
                onClick={(event) => event.stopPropagation()}
              />
            )
          },
          ...(sources.isError || models.isError
            ? [
                {
                  key: `${entry.id}:error`,
                  label: i18nText('frontstage', 'data_blocks.create.retry'),
                  onClick: () => {
                    models.refetch();
                    sources.refetch();
                  }
                }
              ]
            : sources.isLoading || models.isLoading
              ? [
                  {
                    key: `${entry.id}:loading`,
                    disabled: true,
                    label: i18nText('frontstage', 'data_blocks.create.loading')
                  }
                ]
              : sourceGroups.length === 1
                ? sourceGroups[0]!.children
                : sourceGroups.length
                  ? sourceGroups
                  : [
                      {
                        key: `${entry.id}:empty`,
                        disabled: true,
                        label: i18nText(
                          'frontstage',
                          'data_blocks.create.empty'
                        )
                      }
                    ])
        ];
        return {
          key: entry.id,
          label:
            entry.contributionCode === TABLE_BLOCK_CONTRIBUTION
              ? i18nText('frontstage', 'data_blocks.table.table')
              : i18nText('frontstage', 'data_blocks.form.form'),
          children
        };
      })
    });
  }
  if (codeEntries.length)
    items.push({
      key: 'custom',
      type: 'group',
      label: i18nText('frontstage', 'data_blocks.create.custom'),
      children: codeEntries.map((entry) => ({
        key: entry.id,
        label:
          entry.contributionCode === 'frontstage.js-ui-block'
            ? i18nText('frontstage', 'data_blocks.create.js')
            : entry.title,
        onClick: () => choose(entry)
      }))
    });
  if (error)
    items.unshift({
      key: 'catalog-error',
      label: error.message,
      onClick: onRetry
    });
  if (!items.length)
    items.push({
      key: 'empty',
      disabled: true,
      label: i18nText('frontstage', 'auto.no_available_block_catalog_entries')
    });
  return (
    <Dropdown
      open={open}
      onOpenChange={setOpen}
      trigger={['hover', 'click']}
      menu={{ items }}
      disabled={disabled}
    >
      <Button
        size="middle"
        aria-label={i18nText('frontstage', 'auto.create_block')}
        data-flowbase-frontstage-canvas-footer=""
        disabled={disabled}
        loading={loading}
        style={{
          margin: '8px 16px 16px',
          borderStyle: 'dashed',
          borderColor: FRONTSTAGE_DESIGN_BLUE.dashed,
          color: FRONTSTAGE_DESIGN_BLUE.primary
        }}
      >
        {i18nText('frontstage', 'auto.add_block_button')}
      </Button>
    </Dropdown>
  );
}
