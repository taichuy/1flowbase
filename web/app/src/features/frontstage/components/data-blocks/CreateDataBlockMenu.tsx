import { useState, type CSSProperties } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Button, Dropdown, Input, theme, type MenuProps } from 'antd';
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

const MENU_WIDTH: CSSProperties = {
  width: 280,
  minWidth: 0,
  maxWidth: 'calc(100dvw - 24px)'
};
const MENU_PANEL: CSSProperties = {
  ...MENU_WIDTH,
  maxHeight: 'min(480px, calc(100dvh - 24px))',
  overflowY: 'auto',
  overflowX: 'hidden',
  overscrollBehavior: 'contain'
};
const SUBMENU_PLACEMENTS: MenuProps['builtinPlacements'] = {
  rightTop: {
    points: ['tl', 'tr'],
    overflow: { adjustX: 1, adjustY: 1, shiftX: true, shiftY: true }
  },
  leftTop: {
    points: ['tr', 'tl'],
    overflow: { adjustX: 1, adjustY: 1, shiftX: true, shiftY: true }
  }
};

function menuLabel(text: string) {
  return (
    <span
      title={text}
      style={{
        display: 'block',
        overflow: 'hidden',
        textOverflow: 'ellipsis',
        whiteSpace: 'nowrap'
      }}
    >
      {text}
    </span>
  );
}

export function CreateDataBlockMenu({
  workspaceId,
  entries,
  disabled,
  loading,
  error,
  onRetry,
  currentBinding,
  allowOtherModels = true,
  onCreate
}: {
  workspaceId: string;
  entries: readonly NormalizedFrontstageBlockCatalogEntry[];
  disabled: boolean;
  loading: boolean;
  error?: Error | null;
  onRetry?(): void;
  currentBinding?: DataBlockBinding;
  allowOtherModels?: boolean;
  onCreate(
    entry: NormalizedFrontstageBlockCatalogEntry,
    binding?: DataBlockBinding
  ): void | Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const { token } = theme.useToken();
  const actor = useAuthStore((state) => state.actor);
  const models = useQuery({
    queryKey: ['frontstage-design-models', workspaceId, actor?.id],
    queryFn: () => fetchConsoleDataModels({}, getFrontstageApiBaseUrl()),
    enabled: open && allowOtherModels,
    retry: false
  });
  const sources = useQuery({
    queryKey: ['frontstage-design-sources', workspaceId, actor?.id],
    queryFn: () => fetchConsoleDataSources(getFrontstageApiBaseUrl()),
    enabled: open && allowOtherModels,
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
            (!currentBinding || model.id !== currentBinding.model_id) &&
            model.capabilities.record.can_list &&
            `${model.title} ${model.code}`
              .toLowerCase()
              .includes(search.toLowerCase())
        );
        const sourceGroups = (sources.data ?? [])
          .map((source) => ({
            key: `${entry.id}:${source.id}`,
            label: menuLabel(source.display_name),
            popupStyle: MENU_PANEL,
            children: available
              .filter((model) => (model.data_source_id ?? 'main') === source.id)
              .map((model) => ({
                key: `${entry.id}:${model.id}`,
                label: menuLabel(model.title),
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
            style: {
              position: 'sticky',
              top: 0,
              zIndex: 1,
              background: token.colorBgElevated,
              cursor: 'default'
            },
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
          popupStyle: MENU_PANEL,
          label: entry.raw.custom_block_id
            ? menuLabel(entry.title)
            : entry.contributionCode === TABLE_BLOCK_CONTRIBUTION
              ? i18nText('frontstage', 'data_blocks.table.table')
              : i18nText('frontstage', 'data_blocks.form.form'),
          children: currentBinding
            ? [
                {
                  key: `${entry.id}:current`,
                  label: i18nText(
                    'frontstage',
                    'data_blocks.create.current_model'
                  ),
                  onClick: () => choose(entry, currentBinding)
                },
                ...(allowOtherModels
                  ? [
                      {
                        key: `${entry.id}:other`,
                        popupStyle: MENU_PANEL,
                        label: i18nText(
                          'frontstage',
                          'data_blocks.create.other_models'
                        ),
                        children
                      }
                    ]
                  : [])
              ]
            : children
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
          !entry.raw.custom_block_id &&
          entry.contributionCode === 'frontstage.js-ui-block'
            ? i18nText('frontstage', 'data_blocks.create.js')
            : menuLabel(entry.title),
        onClick: () => choose(entry)
      }))
    });
  if (error)
    items.unshift({
      key: 'catalog-error',
      label: menuLabel(error.message),
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
      autoAdjustOverflow
      align={{
        offset: [0, 0],
        overflow: { adjustX: 1, adjustY: 1, shiftX: true, shiftY: true }
      }}
      styles={{
        root: MENU_WIDTH,
        itemTitle: { minWidth: 0 },
        itemContent: { minWidth: 0, overflow: 'hidden', flex: 1 }
      }}
      menu={{
        items,
        style: MENU_PANEL,
        builtinPlacements: SUBMENU_PLACEMENTS
      }}
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
