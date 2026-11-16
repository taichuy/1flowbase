import { Button } from 'antd';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import FilterOutlined from '@ant-design/icons/es/icons/FilterOutlined';
import type { ConsoleRuntimeModelRecord } from '@1flowbase/api-client';
import type { DataBlockAction } from '../../../../lib/data-blocks/types';
import { BlockDesignToolbar } from '../../../../lib/native-modules/block-design/runtime';
import { tableActionAllowed } from '../table-model';
import { actionTitle } from '../table-labels';
import { TableActionSettings } from '../TableSettings';
import type { useTableConfigController } from '../use-table-config-controller';
import type { useTableSession } from '../runtime/use-table-session';
import type { useTableActions } from '../runtime/use-table-actions';

export function TableAction({
  action,
  row = false,
  record,
  session,
  actions,
  configuration,
  disabled,
  design,
  onFilter
}: {
  action: DataBlockAction;
  row?: boolean;
  record?: ConsoleRuntimeModelRecord;
  session: ReturnType<typeof useTableSession>;
  actions: ReturnType<typeof useTableActions>;
  configuration: ReturnType<typeof useTableConfigController>;
  disabled: boolean;
  design: boolean;
  onFilter?: () => void;
}) {
  const allowed =
    tableActionAllowed(action, session.capabilities) &&
    (!(row || action.kind === 'delete') || session.canAddressRecords);
  if (!allowed && !design) return null;
  const icon = row ? undefined : action.kind === 'create' ? (
    <PlusOutlined />
  ) : action.kind === 'delete' ? (
    <DeleteOutlined />
  ) : action.kind === 'refresh' ? (
    <ReloadOutlined />
  ) : (
    <FilterOutlined />
  );
  return (
    <BlockDesignToolbar
      className="data-table-action"
      disabled={!design}
      tools={
        <TableActionSettings
          action={action}
          fields={session.fields}
          variables={session.variables}
          disabled={disabled}
          onSave={(next) => configuration.saveAction(next, row)}
        />
      }
    >
      <Button
        size="small"
        type={row ? 'link' : action.kind === 'create' ? 'primary' : 'default'}
        danger={action.kind === 'delete'}
        icon={icon}
        disabled={
          !allowed ||
          actions.deleting ||
          (action.kind === 'delete' && !record && !session.selected.length) ||
          (['create', 'refresh'].includes(action.kind) &&
            !session.scope.enabled) ||
          (action.kind === 'create' && !actions.actionParent.canCreate) ||
          (action.kind === 'refresh' && session.invalidQuery)
        }
        loading={
          !row && action.kind === 'refresh' && session.records.isFetching
        }
        onClick={() => {
          if (action.kind === 'filter' && onFilter) onFilter();
          else void actions.runAction(action, record);
        }}
      >
        {actionTitle(action)}
      </Button>
    </BlockDesignToolbar>
  );
}
