import { useConsoleOperationAccess } from '../../../../../../state/console-operation-access';
import { useState, type CSSProperties } from 'react';
import { Alert, ConfigProvider, Typography, theme } from 'antd';
import type { BlockContext } from '@1flowbase/page-protocol';
import { i18nText } from '../../../../../../shared/i18n/text';
import type { useDataBlockMetadata } from '../../../../api/data-blocks';
import type { DataBlockHost } from '../../../../lib/data-blocks/host-context';
import {
  BLOCK_DESIGN_THEME,
  BLOCK_BUTTON_THEME
} from '../../../../lib/native-modules/block-design/theme';
import { FRONTSTAGE_DESIGN_BLUE } from '../../../../lib/design-mode-theme';
import { useDataBlockPopupContainer } from '../../shared/useDataBlockPopupContainer';
import { DataBlockModal as Modal } from '../../shared/DataBlockModal';
import { DataFieldCreateModal } from '../../shared/DataFieldCreateModal';
import { TableFieldMenu } from '../TableDesignControls';
import { TableBlockSettings } from '../TableSettings';
import { TableGrid } from './TableGrid';
import { TableToolbar } from './TableToolbar';
import { TableState } from './TableState';
import type { useTableConfigController } from '../use-table-config-controller';
import type { useTableSession } from '../runtime/use-table-session';
import type { useTableActions } from '../runtime/use-table-actions';

export function TableBlockView({
  ctx,
  host,
  configuration,
  session,
  actions,
  metadata,
  feedback,
  onDismissFeedback
}: {
  ctx: BlockContext;
  host: DataBlockHost | null;
  configuration: ReturnType<typeof useTableConfigController>;
  session: ReturnType<typeof useTableSession>;
  actions: ReturnType<typeof useTableActions>;
  metadata: ReturnType<typeof useDataBlockMetadata>;
  feedback: ReturnType<typeof useTableActions>['feedback'];
  onDismissFeedback: () => void;
}) {
  const { config, saving, saveConfig } = configuration;
  const { fields, variables, invalidFields, capabilities, canAddressRecords } =
    session;
  const { deleteIds, deleting, confirmDelete } = actions;
  const [createField, setCreateField] = useState(false);
  const popupContainer = useDataBlockPopupContainer();
  const { token } = theme.useToken();
  const design = Boolean(host?.design_mode);
  const disabled = saving || Boolean(host?.saving) || !metadata.data;
  const can = useConsoleOperationAccess();
  const canCreateField = design && can('model_fields.create');
  const fieldMenu = design && (
    <TableFieldMenu
      fields={fields}
      config={config}
      disabled={disabled || !metadata.data}
      onChange={saveConfig}
      canCreate={canCreateField}
      onCreate={() => setCreateField(true)}
    />
  );
  return (
    <ConfigProvider
      theme={design ? BLOCK_DESIGN_THEME : BLOCK_BUTTON_THEME}
      getPopupContainer={popupContainer}
    >
      <section
        className={`data-table-block${design ? ' data-table-block-design' : ''}`}
        aria-label={
          config.title || i18nText('frontstage', 'data_blocks.table.table')
        }
        style={
          {
            background: token.colorBgContainer,
            color: token.colorText,
            borderRadius: token.borderRadiusLG,
            '--data-table-design-color': FRONTSTAGE_DESIGN_BLUE.primary,
            '--data-table-border-color': token.colorBorderSecondary
          } as CSSProperties
        }
      >
        {(config.title || config.description) && (
          <div className="data-table-block-heading">
            <div>
              {config.title && (
                <Typography.Text strong>{config.title}</Typography.Text>
              )}
              {config.description && (
                <Typography.Paragraph type="secondary">
                  {config.description}
                </Typography.Paragraph>
              )}
            </div>
          </div>
        )}
        {design && host && (
          <TableBlockSettings
            config={config}
            fields={fields}
            variables={variables}
            targets={host.targets}
            blockId={host.block_id}
            disabled={disabled}
            onSave={saveConfig}
          />
        )}
        {feedback && (
          <Alert
            showIcon
            closable
            type={feedback.type}
            title={feedback.text}
            onClose={onDismissFeedback}
          />
        )}
        {metadata.data && !canAddressRecords && (
          <Alert
            type="info"
            showIcon
            title={i18nText(
              'frontstage',
              'data_blocks.table.unaddressable_records'
            )}
          />
        )}
        {invalidFields.length > 0 && (
          <Alert
            type="warning"
            showIcon
            title={i18nText('frontstage', 'data_blocks.table.invalid_columns')}
          />
        )}
        <TableToolbar
          configuration={configuration}
          session={session}
          actions={actions}
          disabled={disabled}
          design={design}
        />
        <TableState
          metadata={metadata}
          session={session}
          fieldMenu={fieldMenu}
          design={design}
        >
          <TableGrid
            configuration={configuration}
            session={session}
            actions={actions}
            disabled={disabled}
            design={design}
            fieldMenu={fieldMenu}
          />
        </TableState>
        <Modal
          getContainer={false}
          open={deleteIds !== null}
          title={i18nText('frontstage', 'data_blocks.table.confirm_delete')}
          onCancel={actions.cancelDelete}
          confirmLoading={deleting}
          closable={!deleting}
          maskClosable={!deleting}
          cancelButtonProps={{ disabled: deleting }}
          okButtonProps={{
            danger: true,
            disabled: !deleteIds?.length || !capabilities?.can_delete
          }}
          okText={i18nText('frontstage', 'auto.delete')}
          cancelText={i18nText('frontstage', 'auto.cancel')}
          onOk={() => void confirmDelete()}
        >
          {i18nText('frontstage', 'data_blocks.table.delete_description', {
            count: deleteIds?.length ?? 0
          })}
        </Modal>
        {canCreateField && (
          <DataFieldCreateModal
            ctx={ctx}
            binding={config.binding}
            open={createField}
            onClose={() => setCreateField(false)}
            onCreated={async (fieldCode) => {
              await metadata.refetch();
              const existing = config.columns.find(
                (column) => column.field_code === fieldCode
              );
              const attached = await saveConfig({
                ...config,
                columns: existing
                  ? config.columns.map((column) =>
                      column.field_code === fieldCode
                        ? { ...column, hidden: false }
                        : column
                    )
                  : [...config.columns, { field_code: fieldCode }]
              });
              if (!attached)
                throw new Error(
                  i18nText(
                    'frontstage',
                    'data_blocks.fields.created_attach_failed'
                  )
                );
            }}
          />
        )}
      </section>
    </ConfigProvider>
  );
}
