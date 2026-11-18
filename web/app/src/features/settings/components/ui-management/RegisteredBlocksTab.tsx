import { useCallback, useEffect, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, App, Button, Descriptions } from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import { selectNavigationQueryScope } from '../../../../state/navigation-query-scope';
import { ResizableDrawer } from '../../../../shared/ui/resizable-drawer/ResizableDrawer';
import {
  fetchSettingsUiBlocks,
  settingsUiBlocksQueryKey,
  fetchSettingsCustomBlocks,
  createSettingsCustomBlock,
  updateSettingsCustomBlock,
  deleteSettingsCustomBlock,
  type ConsoleFrontendBlockCatalogEntry,
  type SettingsUiTemplateInput
} from '../../api/ui-management';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import {
  UiCodeTemplateStudio,
  type UiCodeTemplateStudioMode
} from './UiCodeTemplateStudio';

import {
  RegisteredBlocksTable,
  type BlockRow
} from './registered-blocks/RegisteredBlocksTable';

type StudioSession = {
  documentId: string;
  mode: UiCodeTemplateStudioMode;
  id?: string;
  initialValue: SettingsUiTemplateInput;
};

export function RegisteredBlocksTab() {
  const scope = useAuthStore(selectNavigationQueryScope);
  return <RegisteredBlocksContent key={scope} scope={scope} />;
}

function RegisteredBlocksContent({ scope }: { scope: string }) {
  const { t } = useTranslation('settingsUiManagement');
  const { message } = App.useApp();
  const can = useConsoleOperationAccess();
  const canView = can('frontend_blocks.view');
  const canListCustom = can('ui_management.blocks.list');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id ?? null
  );
  const queryClient = useQueryClient();
  const [studio, setStudio] = useState<StudioSession | null>(null);
  const currentStudio = useRef<StudioSession | null>(null);
  useEffect(
    () => () => {
      currentStudio.current = null;
    },
    []
  );
  const closeStudio = () => {
    currentStudio.current = null;
    setStudio(null);
  };
  const [detail, setDetail] = useState<ConsoleFrontendBlockCatalogEntry | null>(
    null
  );
  const query = useQuery({
    queryKey: [...settingsUiBlocksQueryKey, 'catalog', 'block', scope],
    queryFn: () =>
      fetchSettingsUiBlocks(undefined, { registration_target: 'block' }),
    enabled: canView,
    retry: false
  });
  const custom = useQuery({
    queryKey: [...settingsUiBlocksQueryKey, 'custom', scope],
    queryFn: () => fetchSettingsCustomBlocks(),
    enabled: canListCustom,
    retry: false
  });
  const invalidate = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: settingsUiBlocksQueryKey }),
      queryClient.invalidateQueries({
        queryKey: ['frontstage', 'block-catalog']
      })
    ]);
  }, [queryClient]);
  const save = useMutation({
    mutationFn: async ({
      value,
      session
    }: {
      value: SettingsUiTemplateInput;
      session: StudioSession;
    }) => {
      if (!csrfToken || currentStudio.current !== session)
        throw new Error(t('block_save_failed'));
      const body = {
        title: value.name,
        source: value.source,
        language: value.language
      };
      return session.mode === 'edit' && session.id
        ? updateSettingsCustomBlock(session.id, body, csrfToken)
        : createSettingsCustomBlock(
            {
              ...body,
              provider_code: value.provider_code,
              contribution_code: value.contribution_code
            },
            csrfToken
          );
    },
    onSuccess: async (_, { session }) => {
      if (currentStudio.current === session) {
        currentStudio.current = null;
        setStudio(null);
        void message.success(t('block_saved'));
      }
      await invalidate();
    },
    onError: (_, { session }) => {
      if (currentStudio.current === session)
        void message.error(t('block_save_failed'));
    }
  });
  const remove = useMutation({
    mutationFn: async (id: string) => {
      if (!csrfToken) throw new Error(t('block_delete_failed'));
      await deleteSettingsCustomBlock(id, csrfToken);
    },
    onSuccess: async () => {
      await invalidate();
      void message.success(t('block_deleted'));
    },
    onError: () => void message.error(t('block_delete_failed'))
  });
  const open = useCallback(
    (row: BlockRow, mode: 'view' | 'edit' | 'copy') => {
      const source =
        row.kind === 'native' ? row.record.code_template : row.record.source;
      const language =
        row.kind === 'native'
          ? row.record.code_template_language
          : row.record.language;
      if (!source || !language) {
        if (row.kind === 'native' && mode === 'view') setDetail(row.record);
        return;
      }
      const session: StudioSession = {
        documentId:
          mode === 'copy'
            ? `copy:${crypto.randomUUID()}`
            : row.kind === 'custom'
              ? row.record.id
              : row.record.frontend_contribution_id,
        mode,
        id:
          row.kind === 'custom' && mode !== 'copy' ? row.record.id : undefined,
        initialValue: {
          provider_code: row.record.provider_code,
          contribution_code: row.record.contribution_code,
          name:
            mode === 'copy'
              ? `${row.record.title} - ${t('copy_name_suffix')}`
              : row.record.title,
          source,
          language
        }
      };
      currentStudio.current = session;
      setStudio(session);
    },
    [t]
  );
  const failed = query.isError || custom.isError;
  const refresh = () => {
    if (canView) void query.refetch();
    if (canListCustom) void custom.refetch();
  };
  return (
    <SettingsSectionSurface
      heightMode="fill"
      status={
        !canView && !canListCustom ? (
          <Alert type="warning" showIcon title={t('blocks_no_access')} />
        ) : failed ? (
          <Alert
            type="error"
            showIcon
            title={t('blocks_load_failed')}
            action={
              <Button
                loading={query.isFetching || custom.isFetching}
                onClick={refresh}
              >
                {t('routes_retry')}
              </Button>
            }
          />
        ) : undefined
      }
    >
      {canView || canListCustom ? (
        <RegisteredBlocksTable
          catalog={query.data ?? []}
          custom={custom.data ?? []}
          loading={query.isFetching || custom.isFetching}
          deleting={remove.isPending}
          onOpen={open}
          onDelete={(id) => remove.mutateAsync(id)}
          onRefresh={refresh}
        />
      ) : null}
      {studio ? (
        <UiCodeTemplateStudio
          resourceKind="block"
          documentId={studio.documentId}
          initialValue={studio.initialValue}
          mode={studio.mode}
          officialTemplates={[]}
          open
          saving={save.isPending && save.variables.session === studio}
          workspaceId={workspaceId}
          onClose={closeStudio}
          onSave={(value) => save.mutate({ value, session: studio })}
        />
      ) : null}
      <ResizableDrawer
        title={detail?.title}
        open={Boolean(detail)}
        onClose={() => setDetail(null)}
        defaultWidth={600}
        resizeLabel={t('block_resize')}
      >
        {detail ? (
          <Descriptions
            column={1}
            items={[
              {
                key: 'code',
                label: t('block_code'),
                children: detail.contribution_code
              },
              {
                key: 'source',
                label: t('source'),
                children: detail.provider_code
              },
              {
                key: 'version',
                label: t('resource_version'),
                children: detail.frontend_block_version
              },
              {
                key: 'type',
                label: t('block_type'),
                children: t('block_native')
              }
            ]}
          />
        ) : null}
      </ResizableDrawer>
    </SettingsSectionSurface>
  );
}
